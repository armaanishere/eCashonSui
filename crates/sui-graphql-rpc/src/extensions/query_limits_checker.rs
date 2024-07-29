// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::config::{Limits, ServiceConfig};
use crate::error::{code, graphql_error, graphql_error_at_pos};
use crate::metrics::Metrics;
use async_graphql::extensions::NextParseQuery;
use async_graphql::extensions::NextRequest;
use async_graphql::extensions::{Extension, ExtensionContext, ExtensionFactory};
use async_graphql::parser::types::{
    DocumentOperations, ExecutableDocument, Field, FragmentDefinition, OperationDefinition,
    Selection,
};
use async_graphql::{value, Name, Positioned, Response, ServerError, ServerResult, Variables};
use async_graphql_value::Value as GqlValue;
use async_graphql_value::{ConstValue, Value};
use async_trait::async_trait;
use axum::http::HeaderName;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::mem;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use sui_graphql_rpc_headers::LIMITS_HEADER;
use tracing::info;
use uuid::Uuid;

pub(crate) const CONNECTION_FIELDS: [&str; 2] = ["edges", "nodes"];

#[derive(Debug)]
pub(crate) struct PayloadSize(pub u64);

struct PayloadSizeCheck<'a> {
    variables: &'a Variables,
    max_query_payload_size: u32,
    max_mutation_payload_size: u32,
}

/// Extension factory for adding checks that the query is within configurable limits.
pub(crate) struct QueryLimitsChecker;

#[derive(Debug, Default)]
struct QueryLimitsCheckerExt {
    usage: Mutex<Option<Usage>>,
}

/// Only display usage information if this header was in the request.
pub(crate) struct ShowUsage;

impl<'a> PayloadSizeCheck<'a> {
    fn new(limits: &Limits, variables: &'a Variables) -> Self {
        Self {
            max_query_payload_size: limits.max_query_payload_size,
            max_mutation_payload_size: limits.max_tx_payload_size,
            variables,
        }
    }

    fn check_document(
        &self,
        doc: &ExecutableDocument,
        ctx: &ExtensionContext<'_>,
    ) -> ServerResult<()> {
        self.check_payload_size(ctx)?;
        self.check_mutation_dry_run(doc, ctx)?;
        Ok(())
    }

    fn check_mutation_dry_run(
        &self,
        doc: &ExecutableDocument,
        ctx: &ExtensionContext<'_>,
    ) -> ServerResult<()> {
        let payload_size: &PayloadSize = ctx.data_unchecked();
        let payload_size = payload_size.0;
        // keep track of variables that are used in the dryRunTransactionBlock and
        // executeTransactionBlock. We will remove them from the total query size
        // but we keep them in a map in case one variable is used in multiple places,
        // to avoid over counting
        let mut track_vars = BTreeMap::<String, usize>::new();
        for (_, val) in doc.operations.iter() {
            for n in val.node.selection_set.node.items.iter() {
                if let Selection::Field(f) = &n.node {
                    if f.node.name.node == "dryRunTransactionBlock" {
                        for arg in &f.node.arguments {
                            if arg.0.node == "txBytes" {
                                let tx_bytes_len = get_value_str_len(&arg.1.node, self.variables);
                                if tx_bytes_len > self.max_mutation_payload_size as usize {
                                    self.log_metric(
                                            ctx,
                                            "The txBytes size of dryRunTransactionBlock node is too large: {}",
                                            payload_size
                                        );
                                    return Err(graphql_error_at_pos(
                                            code::BAD_USER_INPUT,
                                            format!(
                                                "The txBytes size of dryRunTransactionBlock node is too large. The maximum allowed is {} bytes",
                                                self.max_mutation_payload_size
                                            ),
                                            f.pos
                                        ));
                                }
                                track_vars.insert(arg.1.node.to_string(), tx_bytes_len);
                            }
                        }
                    } else if f.node.name.node == "executeTransactionBlock" {
                        let mut tx_bytes_len = 0;
                        for arg in &f.node.arguments {
                            tx_bytes_len += get_value_str_len(&arg.1.node, self.variables);
                        }
                        if tx_bytes_len > self.max_mutation_payload_size as usize {
                            self.log_metric(
                                ctx,
                                "The txBytes+signatures size of {} node is too large: {}",
                                payload_size,
                            );
                            return Err(graphql_error_at_pos(
                                    code::BAD_USER_INPUT,
                                    format!(
                                        "The txBytes+signatures size of {} node is too large. The maximum allowed is {} bytes",
                                        f.node.name.node,
                                        self.max_mutation_payload_size
                                    ),
                                    f.pos
                                ));
                        }

                        track_vars.insert(
                            format!(
                                "{}",
                                &f.node
                                    .arguments
                                    .iter()
                                    .map(|x| x.1.node.to_string())
                                    .collect::<Vec<_>>()
                                    .concat(),
                            ),
                            tx_bytes_len,
                        );
                    }
                }
            }
        }

        let cfg = ctx
            .data::<ServiceConfig>()
            .expect("No service config provided in schema data");

        // remove the executeTransactionBlock and the dryRunTransactionBlock variables from the
        // total query size, and check if the rest is bigger than the allowed limit
        if (payload_size - track_vars.values().sum::<usize>() as u64)
            > cfg.limits.max_query_payload_size.into()
        {
            self.log_metric(ctx, "Query payload is too large: {}", payload_size);
            return Err(graphql_error(
                code::BAD_USER_INPUT,
                format!(
                    "Query payload is too large. The maximum allowed is {} bytes",
                    cfg.limits.max_query_payload_size
                ),
            ));
        }

        Ok(())
    }

    /// Check the whole query (content-length) against the allowed payload.
    fn check_payload_size(&self, ctx: &ExtensionContext<'_>) -> ServerResult<()> {
        let payload_size: &PayloadSize = ctx.data_unchecked();
        let payload_size = payload_size.0;

        if let Ok(max_payload_size) =
            u64::try_from(self.max_query_payload_size + self.max_mutation_payload_size)
        {
            if payload_size > max_payload_size {
                let metrics: &Metrics = ctx.data_unchecked();
                let query_id: &Uuid = ctx.data_unchecked();
                let session_id: &SocketAddr = ctx.data_unchecked();
                metrics
                    .request_metrics
                    .query_payload_too_large_size
                    .observe(payload_size as f64);
                info!(
                    query_id = %query_id,
                    session_id = %session_id,
                    error_code = code::BAD_USER_INPUT,
                    "Query payload is too large: {}",
                    payload_size
                );
                return Err(graphql_error(
                    code::BAD_USER_INPUT,
                    format!("Query payload too large. Max allowed is {max_payload_size}"),
                ));
            }
        }
        Ok(())
    }

    fn log_metric(&self, ctx: &ExtensionContext<'_>, message: &str, payload_size: u64) {
        let metrics: &Metrics = ctx.data_unchecked();
        let query_id: &Uuid = ctx.data_unchecked();
        let session_id: &SocketAddr = ctx.data_unchecked();
        metrics
            .request_metrics
            .query_payload_too_large_size
            .observe(payload_size as f64);
        info!(
            query_id = %query_id,
            session_id = %session_id,
            error_code = code::BAD_USER_INPUT,
            message,
            payload_size
        );
    }
}

/// State for traversing a document to check for limits. Holds on to environments for looking up
/// variables and fragments, limits, and the remainder of the limit that can be used.
struct LimitsTraversal<'a> {
    // Environments for resolving lookups in the document
    fragments: &'a HashMap<Name, Positioned<FragmentDefinition>>,
    variables: &'a Variables,

    // Relevant limits from the service configuration
    default_page_size: u32,
    max_input_nodes: u32,
    max_output_nodes: u32,
    max_depth: u32,

    // Remaining budget for the traversal
    input_budget: u32,
    output_budget: u32,
    depth_seen: u32,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Usage {
    input_nodes: u32,
    output_nodes: u32,
    depth: u32,
    variables: u32,
    fragments: u32,
    query_payload: u32,
}

impl ShowUsage {
    pub(crate) fn name() -> &'static HeaderName {
        &LIMITS_HEADER
    }
}

impl<'a> LimitsTraversal<'a> {
    fn new(
        limits: &Limits,
        fragments: &'a HashMap<Name, Positioned<FragmentDefinition>>,
        variables: &'a Variables,
    ) -> Self {
        Self {
            fragments,
            variables,
            default_page_size: limits.default_page_size,
            max_input_nodes: limits.max_query_nodes,
            max_output_nodes: limits.max_output_nodes,
            max_depth: limits.max_query_depth,
            input_budget: limits.max_query_nodes,
            output_budget: limits.max_output_nodes,
            depth_seen: 0,
        }
    }

    /// Main entrypoint for checking all limits.
    fn check_document(&mut self, doc: &ExecutableDocument) -> ServerResult<()> {
        for (_name, op) in doc.operations.iter() {
            self.check_input_limits(op)?;
            self.check_output_limits(op)?;
        }
        Ok(())
    }

    /// Test that the operation meets input limits (number of nodes and depth).
    fn check_input_limits(&mut self, op: &Positioned<OperationDefinition>) -> ServerResult<()> {
        let mut next_level = vec![];
        let mut curr_level = vec![];
        let mut depth_budget = self.max_depth;

        next_level.extend(&op.node.selection_set.node.items);
        while let Some(next) = next_level.first() {
            if depth_budget == 0 {
                return Err(graphql_error_at_pos(
                    code::BAD_USER_INPUT,
                    format!("Query nesting is over {}", self.max_depth),
                    next.pos,
                ));
            } else {
                depth_budget -= 1;
            }

            mem::swap(&mut next_level, &mut curr_level);

            for selection in curr_level.drain(..) {
                if self.input_budget == 0 {
                    return Err(graphql_error_at_pos(
                        code::BAD_USER_INPUT,
                        format!("Query has over {} nodes", self.max_input_nodes),
                        selection.pos,
                    ));
                } else {
                    self.input_budget -= 1;
                }

                match &selection.node {
                    Selection::Field(f) => {
                        next_level.extend(&f.node.selection_set.node.items);
                    }

                    Selection::InlineFragment(f) => {
                        next_level.extend(&f.node.selection_set.node.items);
                    }

                    Selection::FragmentSpread(fs) => {
                        let name = &fs.node.fragment_name.node;
                        let def = self.fragments.get(name).ok_or_else(|| {
                            graphql_error_at_pos(
                                code::INTERNAL_SERVER_ERROR,
                                format!("Fragment {name} referred to but not found in document"),
                                fs.pos,
                            )
                        })?;

                        next_level.extend(&def.node.selection_set.node.items);
                    }
                }
            }
        }

        self.depth_seen = self.depth_seen.max(self.max_depth - depth_budget);
        Ok(())
    }

    /// Check that the operation's output node estimate will not exceed the service's limit.
    ///
    /// This check must be done after the input limit check, because it relies on the query depth
    /// being bounded to protect it from recursing too deeply.
    fn check_output_limits(&mut self, op: &Positioned<OperationDefinition>) -> ServerResult<()> {
        for selection in &op.node.selection_set.node.items {
            self.traverse_selection_for_output(selection, 1, None)?;
        }
        Ok(())
    }

    /// Account for the estimated output size of this selection and its children.
    ///
    /// `multiplicity` is the number of times this selection will be output, on account of being
    /// nested within paginated ancestors.
    ///
    /// If this field is inside a connection, but not inside one of its fields, `page_size` is the
    /// size of the connection's page.
    fn traverse_selection_for_output(
        &mut self,
        selection: &Positioned<Selection>,
        multiplicity: u32,
        page_size: Option<u32>,
    ) -> ServerResult<()> {
        match &selection.node {
            Selection::Field(f) => {
                if multiplicity > self.output_budget {
                    return Err(self.output_node_error());
                } else {
                    self.output_budget -= multiplicity;
                }

                // If the field being traversed is a connection field, increase multiplicity by a
                // factor of page size. This operation can fail due to overflow, which will be
                // treated as a limits check failure, even if the resulting value does not get used
                // for anything.
                let name = &f.node.name.node;
                let multiplicity = 'm: {
                    if !CONNECTION_FIELDS.contains(&name.as_str()) {
                        break 'm multiplicity;
                    }

                    let Some(page_size) = page_size else {
                        break 'm multiplicity;
                    };

                    multiplicity
                        .checked_mul(page_size)
                        .ok_or_else(|| self.output_node_error())?
                };

                let page_size = self.connection_page_size(f)?;
                for selection in &f.node.selection_set.node.items {
                    self.traverse_selection_for_output(selection, multiplicity, page_size)?;
                }
            }

            // Just recurse through fragments, because they are inlined into their "call site".
            Selection::InlineFragment(f) => {
                for selection in f.node.selection_set.node.items.iter() {
                    self.traverse_selection_for_output(selection, multiplicity, page_size)?;
                }
            }

            Selection::FragmentSpread(fs) => {
                let name = &fs.node.fragment_name.node;
                let def = self.fragments.get(name).ok_or_else(|| {
                    graphql_error_at_pos(
                        code::INTERNAL_SERVER_ERROR,
                        format!("Fragment {name} referred to but not found in document"),
                        fs.pos,
                    )
                })?;

                for selection in def.node.selection_set.node.items.iter() {
                    self.traverse_selection_for_output(selection, multiplicity, page_size)?;
                }
            }
        }

        Ok(())
    }

    /// If the field `f` is a connection, extract its page size, otherwise return `None`.
    /// Returns an error if the page size cannot be represented as a `u32`.
    fn connection_page_size(&mut self, f: &Positioned<Field>) -> ServerResult<Option<u32>> {
        if !self.is_connection(f) {
            return Ok(None);
        }

        let first = f.node.get_argument("first");
        let last = f.node.get_argument("last");

        let page_size = match (self.resolve_u64(first), self.resolve_u64(last)) {
            (Some(f), Some(l)) => f.max(l),
            (Some(p), _) | (_, Some(p)) => p,
            (None, None) => self.default_page_size as u64,
        };

        Ok(Some(
            page_size.try_into().map_err(|_| self.output_node_error())?,
        ))
    }

    /// Checks if the given field corresponds to a connection based on whether it contains a
    /// selection for `edges` or `nodes`. That selection could be immediately in that field's
    /// selection set, or nested within a fragment or inline fragment spread.
    fn is_connection(&self, f: &Positioned<Field>) -> bool {
        f.node
            .selection_set
            .node
            .items
            .iter()
            .any(|s| self.has_connection_fields(s))
    }

    /// Look for fields that suggest the container for this selection is a connection. Recurses
    /// through fragment and inline fragment applications, but does not look recursively through
    /// fields, as only the fields requested from the immediate parent are relevant.
    fn has_connection_fields(&self, s: &Positioned<Selection>) -> bool {
        match &s.node {
            Selection::Field(f) => {
                let name = &f.node.name.node;
                CONNECTION_FIELDS.contains(&name.as_str())
            }

            Selection::InlineFragment(f) => f
                .node
                .selection_set
                .node
                .items
                .iter()
                .any(|s| self.has_connection_fields(s)),

            Selection::FragmentSpread(fs) => {
                let name = &fs.node.fragment_name.node;
                let Some(def) = self.fragments.get(name) else {
                    return false;
                };

                def.node
                    .selection_set
                    .node
                    .items
                    .iter()
                    .any(|s| self.has_connection_fields(s))
            }
        }
    }

    /// Translate a GraphQL value into a u64, if possible, resolving variables if necessary.
    fn resolve_u64(&self, value: Option<&Positioned<Value>>) -> Option<u64> {
        match &value?.node {
            Value::Number(num) => num,

            Value::Variable(var) => {
                if let ConstValue::Number(num) = self.variables.get(var)? {
                    num
                } else {
                    return None;
                }
            }

            _ => return None,
        }
        .as_u64()
    }

    /// Error returned if output node estimate exceeds limit. Also sets the output budget to zero,
    /// to indicate that it has been spent (This is done because unlike other budgets, the output
    /// budget is not decremented one unit at a time, so we can have hit the limit previously but
    /// still have budget left over).
    fn output_node_error(&mut self) -> ServerError {
        self.output_budget = 0;
        graphql_error(
            code::BAD_USER_INPUT,
            format!("Estimated output nodes exceeds {}", self.max_output_nodes),
        )
    }

    /// Finish the traversal and report its usage.
    fn finish(self, query_payload: u32) -> Usage {
        Usage {
            input_nodes: self.max_input_nodes - self.input_budget,
            output_nodes: self.max_output_nodes - self.output_budget,
            depth: self.depth_seen,
            variables: self.variables.len() as u32,
            fragments: self.fragments.len() as u32,
            query_payload,
        }
    }
}

impl Usage {
    fn report(&self, metrics: &Metrics) {
        metrics
            .request_metrics
            .input_nodes
            .observe(self.input_nodes as f64);
        metrics
            .request_metrics
            .output_nodes
            .observe(self.output_nodes as f64);
        metrics
            .request_metrics
            .query_depth
            .observe(self.depth as f64);
        metrics
            .request_metrics
            .query_payload_size
            .observe(self.query_payload as f64);
    }
}

impl ExtensionFactory for QueryLimitsChecker {
    fn create(&self) -> Arc<dyn Extension> {
        Arc::new(QueryLimitsCheckerExt {
            usage: Mutex::new(None),
        })
    }
}

#[async_trait]
impl Extension for QueryLimitsCheckerExt {
    async fn request(&self, ctx: &ExtensionContext<'_>, next: NextRequest<'_>) -> Response {
        let resp = next.run(ctx).await;
        let usage = self.usage.lock().unwrap().take();
        if let Some(usage) = usage {
            resp.extension("usage", value!(usage))
        } else {
            resp
        }
    }

    /// Validates the query against the limits set in the service config
    /// If the limits are hit, the operation terminates early
    async fn parse_query(
        &self,
        ctx: &ExtensionContext<'_>,
        query: &str,
        variables: &Variables,
        next: NextParseQuery<'_>,
    ) -> ServerResult<ExecutableDocument> {
        let metrics: &Metrics = ctx.data_unchecked();
        let instant = Instant::now();

        let cfg = ctx
            .data::<ServiceConfig>()
            .expect("No service config provided in schema data");

        // Document layout of the query
        let doc = next.run(ctx, query, variables).await?;

        // If the query is pure introspection, we don't need to check the limits. Pure introspection
        // queries are queries that only have one operation with one field and that field is a
        // `__schema` query
        if let DocumentOperations::Single(op) = &doc.operations {
            if let [field] = &op.node.selection_set.node.items[..] {
                if let Selection::Field(f) = &field.node {
                    if f.node.name.node == "__schema" {
                        return Ok(doc);
                    }
                }
            }
        }

        let payload_checker = PayloadSizeCheck::new(&cfg.limits, variables);
        payload_checker.check_document(&doc, ctx)?;

        let mut traversal = LimitsTraversal::new(&cfg.limits, &doc.fragments, variables);
        let res = traversal.check_document(&doc);
        let usage = traversal.finish(query.len() as u32);
        metrics.query_validation_latency(instant.elapsed());
        usage.report(metrics);

        res.map(|()| {
            if ctx.data_opt::<ShowUsage>().is_some() {
                *self.usage.lock().unwrap() = Some(usage);
            }

            doc
        })
    }
}

/// Get the length of a string value. If the value is a list, then we expect
/// the list to contain strings, and we sum the lengths of all the strings.
///
/// This is specifically designed to work with the txBytes and signatures
/// of the executeTransactionBlock and dryRunTransactionBlock nodes, which are strings or list of
/// strings.
fn get_value_str_len(arg: &GqlValue, variables: &Variables) -> usize {
    match arg {
        GqlValue::String(s) => s.len(),
        GqlValue::List(arr) => arr.iter().map(|v| get_value_str_len(v, variables)).sum(),
        // the variables are expected to be strings
        GqlValue::Variable(v) => match variables.get(v) {
            Some(value) => get_value_str_len(&value.clone().into_value(), variables),
            None => 0,
        },
        _ => 0,
    }
}
