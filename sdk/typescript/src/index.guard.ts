// Copyright (c) 2022, Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

/*
 * Generated type guards for "index.ts".
 * WARNING: Do not manually change this file.
 */
import { Ed25519KeypairData, Keypair, PublicKeyInitData, PublicKeyData, SignatureScheme, TransferObjectTransaction, TransferSuiTransaction, MergeCoinTransaction, SplitCoinTransaction, MoveCallTransaction, PublishTransaction, TxnDataSerializer, SignaturePubkeyPair, Signer, TransactionDigest, SuiAddress, ObjectOwner, SuiObjectRef, SuiObjectInfo, ObjectContentFields, MovePackageContent, SuiData, SuiMoveObject, SuiMovePackage, SuiMoveFunctionArgTypesResponse, SuiMoveFunctionArgType, SuiMoveFunctionArgTypes, SuiMoveNormalizedModules, SuiMoveNormalizedModule, SuiMoveModuleId, SuiMoveNormalizedStruct, SuiMoveStructTypeParameter, SuiMoveNormalizedField, SuiMoveNormalizedFunction, SuiMoveVisibility, SuiMoveTypeParameterIndex, SuiMoveAbilitySet, SuiMoveNormalizedType, SuiObject, ObjectStatus, ObjectType, GetOwnedObjectsResponse, GetObjectDataResponse, ObjectDigest, ObjectId, SequenceNumber, MoveEvent, PublishEvent, TransferObjectEvent, DeleteObjectEvent, NewObjectEvent, SuiEvent, TransferObject, SuiTransferSui, SuiChangeEpoch, TransactionKindName, SuiTransactionKind, SuiTransactionData, EpochId, AuthorityQuorumSignInfo, CertifiedTransaction, GasCostSummary, ExecutionStatusType, ExecutionStatus, OwnedObjectRef, TransactionEffects, SuiTransactionResponse, GatewayTxSeqNumber, GetTxnDigestsResponse, MoveCall, SuiJsonValue, EmptySignInfo, AuthorityName, AuthoritySignature, TransactionBytes, SuiParsedMergeCoinResponse, SuiParsedSplitCoinResponse, SuiParsedPublishResponse, SuiPackage, SuiParsedTransactionResponse } from "./index";
import { BN } from "bn.js";
import { Base64DataBuffer } from "./serialization/base64";
import { PublicKey } from "./cryptography/publickey";

export function isEd25519KeypairData(obj: any, _argumentName?: string): obj is Ed25519KeypairData {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        obj.publicKey instanceof Uint8Array &&
        obj.secretKey instanceof Uint8Array
    )
}

export function isKeypair(obj: any, _argumentName?: string): obj is Keypair {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        typeof obj.getPublicKey === "function" &&
        typeof obj.signData === "function"
    )
}

export function isPublicKeyInitData(obj: any, _argumentName?: string): obj is PublicKeyInitData {
    return (
        (isTransactionDigest(obj) as boolean ||
            isSuiMoveTypeParameterIndex(obj) as boolean ||
            obj instanceof Buffer ||
            obj instanceof Uint8Array ||
            Array.isArray(obj) &&
            obj.every((e: any) =>
                isSuiMoveTypeParameterIndex(e) as boolean
            ) ||
            isPublicKeyData(obj) as boolean)
    )
}

export function isPublicKeyData(obj: any, _argumentName?: string): obj is PublicKeyData {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        obj._bn instanceof BN
    )
}

export function isSignatureScheme(obj: any, _argumentName?: string): obj is SignatureScheme {
    return (
        (obj === "ED25519" ||
            obj === "Secp256k1")
    )
}

export function isTransferObjectTransaction(obj: any, _argumentName?: string): obj is TransferObjectTransaction {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        isTransactionDigest(obj.objectId) as boolean &&
        (typeof obj.gasPayment === "undefined" ||
            isTransactionDigest(obj.gasPayment) as boolean) &&
        isSuiMoveTypeParameterIndex(obj.gasBudget) as boolean &&
        isTransactionDigest(obj.recipient) as boolean
    )
}

export function isTransferSuiTransaction(obj: any, _argumentName?: string): obj is TransferSuiTransaction {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        isTransactionDigest(obj.suiObjectId) as boolean &&
        isSuiMoveTypeParameterIndex(obj.gasBudget) as boolean &&
        isTransactionDigest(obj.recipient) as boolean &&
        (typeof obj.amount === "undefined" ||
            isSuiMoveTypeParameterIndex(obj.amount) as boolean)
    )
}

export function isMergeCoinTransaction(obj: any, _argumentName?: string): obj is MergeCoinTransaction {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        isTransactionDigest(obj.primaryCoin) as boolean &&
        isTransactionDigest(obj.coinToMerge) as boolean &&
        (typeof obj.gasPayment === "undefined" ||
            isTransactionDigest(obj.gasPayment) as boolean) &&
        isSuiMoveTypeParameterIndex(obj.gasBudget) as boolean
    )
}

export function isSplitCoinTransaction(obj: any, _argumentName?: string): obj is SplitCoinTransaction {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        isTransactionDigest(obj.coinObjectId) as boolean &&
        Array.isArray(obj.splitAmounts) &&
        obj.splitAmounts.every((e: any) =>
            isSuiMoveTypeParameterIndex(e) as boolean
        ) &&
        (typeof obj.gasPayment === "undefined" ||
            isTransactionDigest(obj.gasPayment) as boolean) &&
        isSuiMoveTypeParameterIndex(obj.gasBudget) as boolean
    )
}

export function isMoveCallTransaction(obj: any, _argumentName?: string): obj is MoveCallTransaction {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        isTransactionDigest(obj.packageObjectId) as boolean &&
        isTransactionDigest(obj.module) as boolean &&
        isTransactionDigest(obj.function) as boolean &&
        Array.isArray(obj.typeArguments) &&
        obj.typeArguments.every((e: any) =>
            isTransactionDigest(e) as boolean
        ) &&
        Array.isArray(obj.arguments) &&
        obj.arguments.every((e: any) =>
            isSuiJsonValue(e) as boolean
        ) &&
        (typeof obj.gasPayment === "undefined" ||
            isTransactionDigest(obj.gasPayment) as boolean) &&
        isSuiMoveTypeParameterIndex(obj.gasBudget) as boolean
    )
}

export function isPublishTransaction(obj: any, _argumentName?: string): obj is PublishTransaction {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        Array.isArray(obj.compiledModules) &&
        obj.compiledModules.every((e: any) =>
            isTransactionDigest(e) as boolean
        ) &&
        (typeof obj.gasPayment === "undefined" ||
            isTransactionDigest(obj.gasPayment) as boolean) &&
        isSuiMoveTypeParameterIndex(obj.gasBudget) as boolean
    )
}

export function isTxnDataSerializer(obj: any, _argumentName?: string): obj is TxnDataSerializer {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        typeof obj.newTransferObject === "function" &&
        typeof obj.newTransferSui === "function" &&
        typeof obj.newMoveCall === "function" &&
        typeof obj.newMergeCoin === "function" &&
        typeof obj.newSplitCoin === "function" &&
        typeof obj.newPublish === "function"
    )
}

export function isSignaturePubkeyPair(obj: any, _argumentName?: string): obj is SignaturePubkeyPair {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        isSignatureScheme(obj.signatureScheme) as boolean &&
        obj.signature instanceof Base64DataBuffer &&
        obj.pubKey instanceof PublicKey
    )
}

export function isSigner(obj: any, _argumentName?: string): obj is Signer {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        typeof obj.getAddress === "function" &&
        typeof obj.signData === "function"
    )
}

export function isTransactionDigest(obj: any, _argumentName?: string): obj is TransactionDigest {
    return (
        typeof obj === "string"
    )
}

export function isSuiAddress(obj: any, _argumentName?: string): obj is SuiAddress {
    return (
        typeof obj === "string"
    )
}

export function isObjectOwner(obj: any, _argumentName?: string): obj is ObjectOwner {
    return (
        ((obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
            isTransactionDigest(obj.AddressOwner) as boolean ||
            (obj !== null &&
                typeof obj === "object" ||
                typeof obj === "function") &&
            isTransactionDigest(obj.ObjectOwner) as boolean ||
            (obj !== null &&
                typeof obj === "object" ||
                typeof obj === "function") &&
            isTransactionDigest(obj.SingleOwner) as boolean ||
            obj === "Shared" ||
            obj === "Immutable")
    )
}

export function isSuiObjectRef(obj: any, _argumentName?: string): obj is SuiObjectRef {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        isTransactionDigest(obj.digest) as boolean &&
        isTransactionDigest(obj.objectId) as boolean &&
        isSuiMoveTypeParameterIndex(obj.version) as boolean
    )
}

export function isSuiObjectInfo(obj: any, _argumentName?: string): obj is SuiObjectInfo {
    return (
        isSuiObjectRef(obj) as boolean &&
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        isTransactionDigest(obj.type) as boolean &&
        isObjectOwner(obj.owner) as boolean &&
        isTransactionDigest(obj.previousTransaction) as boolean
    )
}

export function isObjectContentFields(obj: any, _argumentName?: string): obj is ObjectContentFields {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        Object.entries<any>(obj)
            .every(([key, _value]) => (isTransactionDigest(key) as boolean))
    )
}

export function isMovePackageContent(obj: any, _argumentName?: string): obj is MovePackageContent {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        Object.entries<any>(obj)
            .every(([key, value]) => (isTransactionDigest(value) as boolean &&
                isTransactionDigest(key) as boolean))
    )
}

export function isSuiData(obj: any, _argumentName?: string): obj is SuiData {
    return (
        ((obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
            isObjectType(obj.dataType) as boolean &&
            isSuiMoveObject(obj) as boolean ||
            (obj !== null &&
                typeof obj === "object" ||
                typeof obj === "function") &&
            isObjectType(obj.dataType) as boolean &&
            isSuiMovePackage(obj) as boolean)
    )
}

export function isSuiMoveObject(obj: any, _argumentName?: string): obj is SuiMoveObject {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        isTransactionDigest(obj.type) as boolean &&
        isObjectContentFields(obj.fields) as boolean &&
        (typeof obj.has_public_transfer === "undefined" ||
            obj.has_public_transfer === false ||
            obj.has_public_transfer === true)
    )
}

export function isSuiMovePackage(obj: any, _argumentName?: string): obj is SuiMovePackage {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        isMovePackageContent(obj.disassembled) as boolean
    )
}

export function isSuiMoveFunctionArgTypesResponse(obj: any, _argumentName?: string): obj is SuiMoveFunctionArgTypesResponse {
    return (
        Array.isArray(obj) &&
        obj.every((e: any) =>
            isSuiMoveFunctionArgType(e) as boolean
        )
    )
}

export function isSuiMoveFunctionArgType(obj: any, _argumentName?: string): obj is SuiMoveFunctionArgType {
    return (
        (isTransactionDigest(obj) as boolean ||
            (obj !== null &&
                typeof obj === "object" ||
                typeof obj === "function") &&
            isTransactionDigest(obj.Object) as boolean)
    )
}

export function isSuiMoveFunctionArgTypes(obj: any, _argumentName?: string): obj is SuiMoveFunctionArgTypes {
    return (
        Array.isArray(obj) &&
        obj.every((e: any) =>
            isSuiMoveFunctionArgType(e) as boolean
        )
    )
}

export function isSuiMoveNormalizedModules(obj: any, _argumentName?: string): obj is SuiMoveNormalizedModules {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        Object.entries<any>(obj)
            .every(([key, value]) => (isSuiMoveNormalizedModule(value) as boolean &&
                isTransactionDigest(key) as boolean))
    )
}

export function isSuiMoveNormalizedModule(obj: any, _argumentName?: string): obj is SuiMoveNormalizedModule {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        isSuiMoveTypeParameterIndex(obj.file_format_version) as boolean &&
        isTransactionDigest(obj.address) as boolean &&
        isTransactionDigest(obj.name) as boolean &&
        Array.isArray(obj.friends) &&
        obj.friends.every((e: any) =>
            isSuiMoveModuleId(e) as boolean
        ) &&
        (obj.structs !== null &&
            typeof obj.structs === "object" ||
            typeof obj.structs === "function") &&
        Object.entries<any>(obj.structs)
            .every(([key, value]) => (isSuiMoveNormalizedStruct(value) as boolean &&
                isTransactionDigest(key) as boolean)) &&
        (obj.exposed_functions !== null &&
            typeof obj.exposed_functions === "object" ||
            typeof obj.exposed_functions === "function") &&
        Object.entries<any>(obj.exposed_functions)
            .every(([key, value]) => (isSuiMoveNormalizedFunction(value) as boolean &&
                isTransactionDigest(key) as boolean))
    )
}

export function isSuiMoveModuleId(obj: any, _argumentName?: string): obj is SuiMoveModuleId {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        isTransactionDigest(obj.address) as boolean &&
        isTransactionDigest(obj.name) as boolean
    )
}

export function isSuiMoveNormalizedStruct(obj: any, _argumentName?: string): obj is SuiMoveNormalizedStruct {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        isSuiMoveAbilitySet(obj.abilities) as boolean &&
        Array.isArray(obj.type_parameters) &&
        obj.type_parameters.every((e: any) =>
            isSuiMoveStructTypeParameter(e) as boolean
        ) &&
        Array.isArray(obj.fields) &&
        obj.fields.every((e: any) =>
            isSuiMoveNormalizedField(e) as boolean
        )
    )
}

export function isSuiMoveStructTypeParameter(obj: any, _argumentName?: string): obj is SuiMoveStructTypeParameter {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        isSuiMoveAbilitySet(obj.constraints) as boolean &&
        typeof obj.is_phantom === "boolean"
    )
}

export function isSuiMoveNormalizedField(obj: any, _argumentName?: string): obj is SuiMoveNormalizedField {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        isTransactionDigest(obj.name) as boolean &&
        isSuiMoveNormalizedType(obj.type_) as boolean
    )
}

export function isSuiMoveNormalizedFunction(obj: any, _argumentName?: string): obj is SuiMoveNormalizedFunction {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        isSuiMoveVisibility(obj.visibility) as boolean &&
        typeof obj.is_entry === "boolean" &&
        Array.isArray(obj.type_parameters) &&
        obj.type_parameters.every((e: any) =>
            isSuiMoveAbilitySet(e) as boolean
        ) &&
        Array.isArray(obj.parameters) &&
        obj.parameters.every((e: any) =>
            isSuiMoveNormalizedType(e) as boolean
        ) &&
        Array.isArray(obj.return_) &&
        obj.return_.every((e: any) =>
            isSuiMoveNormalizedType(e) as boolean
        )
    )
}

export function isSuiMoveVisibility(obj: any, _argumentName?: string): obj is SuiMoveVisibility {
    return (
        (obj === "Private" ||
            obj === "Public" ||
            obj === "Friend")
    )
}

export function isSuiMoveTypeParameterIndex(obj: any, _argumentName?: string): obj is SuiMoveTypeParameterIndex {
    return (
        typeof obj === "number"
    )
}

export function isSuiMoveAbilitySet(obj: any, _argumentName?: string): obj is SuiMoveAbilitySet {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        Array.isArray(obj.abilities) &&
        obj.abilities.every((e: any) =>
            isTransactionDigest(e) as boolean
        )
    )
}

export function isSuiMoveNormalizedType(obj: any, _argumentName?: string): obj is SuiMoveNormalizedType {
    return (
        (isTransactionDigest(obj) as boolean ||
            (obj !== null &&
                typeof obj === "object" ||
                typeof obj === "function") &&
            isSuiMoveTypeParameterIndex(obj.TypeParameter) as boolean ||
            (obj !== null &&
                typeof obj === "object" ||
                typeof obj === "function") &&
            isSuiMoveNormalizedType(obj.Reference) as boolean ||
            (obj !== null &&
                typeof obj === "object" ||
                typeof obj === "function") &&
            isSuiMoveNormalizedType(obj.MutableReference) as boolean ||
            (obj !== null &&
                typeof obj === "object" ||
                typeof obj === "function") &&
            isSuiMoveNormalizedType(obj.Vector) as boolean ||
            (obj !== null &&
                typeof obj === "object" ||
                typeof obj === "function") &&
            (obj.Struct !== null &&
                typeof obj.Struct === "object" ||
                typeof obj.Struct === "function") &&
            isTransactionDigest(obj.Struct.address) as boolean &&
            isTransactionDigest(obj.Struct.module) as boolean &&
            isTransactionDigest(obj.Struct.name) as boolean &&
            Array.isArray(obj.Struct.type_arguments) &&
            obj.Struct.type_arguments.every((e: any) =>
                isSuiMoveNormalizedType(e) as boolean
            ))
    )
}

export function isSuiObject(obj: any, _argumentName?: string): obj is SuiObject {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        isSuiData(obj.data) as boolean &&
        isObjectOwner(obj.owner) as boolean &&
        isTransactionDigest(obj.previousTransaction) as boolean &&
        isSuiMoveTypeParameterIndex(obj.storageRebate) as boolean &&
        isSuiObjectRef(obj.reference) as boolean
    )
}

export function isObjectStatus(obj: any, _argumentName?: string): obj is ObjectStatus {
    return (
        (obj === "Exists" ||
            obj === "NotExists" ||
            obj === "Deleted")
    )
}

export function isObjectType(obj: any, _argumentName?: string): obj is ObjectType {
    return (
        (obj === "moveObject" ||
            obj === "package")
    )
}

export function isGetOwnedObjectsResponse(obj: any, _argumentName?: string): obj is GetOwnedObjectsResponse {
    return (
        Array.isArray(obj) &&
        obj.every((e: any) =>
            isSuiObjectInfo(e) as boolean
        )
    )
}

export function isGetObjectDataResponse(obj: any, _argumentName?: string): obj is GetObjectDataResponse {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        isObjectStatus(obj.status) as boolean &&
        (isTransactionDigest(obj.details) as boolean ||
            isSuiObjectRef(obj.details) as boolean ||
            isSuiObject(obj.details) as boolean)
    )
}

export function isObjectDigest(obj: any, _argumentName?: string): obj is ObjectDigest {
    return (
        typeof obj === "string"
    )
}

export function isObjectId(obj: any, _argumentName?: string): obj is ObjectId {
    return (
        typeof obj === "string"
    )
}

export function isSequenceNumber(obj: any, _argumentName?: string): obj is SequenceNumber {
    return (
        typeof obj === "number"
    )
}

export function isMoveEvent(obj: any, _argumentName?: string): obj is MoveEvent {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        isTransactionDigest(obj.packageId) as boolean &&
        isTransactionDigest(obj.transactionModule) as boolean &&
        isTransactionDigest(obj.sender) as boolean &&
        isTransactionDigest(obj.type) as boolean &&
        (obj.fields !== null &&
            typeof obj.fields === "object" ||
            typeof obj.fields === "function") &&
        isTransactionDigest(obj.bcs) as boolean
    )
}

export function isPublishEvent(obj: any, _argumentName?: string): obj is PublishEvent {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        isTransactionDigest(obj.sender) as boolean &&
        isTransactionDigest(obj.packageId) as boolean
    )
}

export function isTransferObjectEvent(obj: any, _argumentName?: string): obj is TransferObjectEvent {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        isTransactionDigest(obj.packageId) as boolean &&
        isTransactionDigest(obj.transactionModule) as boolean &&
        isTransactionDigest(obj.sender) as boolean &&
        isObjectOwner(obj.recipient) as boolean &&
        isTransactionDigest(obj.objectId) as boolean &&
        isSuiMoveTypeParameterIndex(obj.version) as boolean &&
        isTransactionDigest(obj.type) as boolean
    )
}

export function isDeleteObjectEvent(obj: any, _argumentName?: string): obj is DeleteObjectEvent {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        isTransactionDigest(obj.packageId) as boolean &&
        isTransactionDigest(obj.transactionModule) as boolean &&
        isTransactionDigest(obj.sender) as boolean &&
        isTransactionDigest(obj.objectId) as boolean
    )
}

export function isNewObjectEvent(obj: any, _argumentName?: string): obj is NewObjectEvent {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        isTransactionDigest(obj.packageId) as boolean &&
        isTransactionDigest(obj.transactionModule) as boolean &&
        isTransactionDigest(obj.sender) as boolean &&
        isObjectOwner(obj.recipient) as boolean &&
        isTransactionDigest(obj.objectId) as boolean
    )
}

export function isSuiEvent(obj: any, _argumentName?: string): obj is SuiEvent {
    return (
        ((obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
            isMoveEvent(obj.moveEvent) as boolean ||
            (obj !== null &&
                typeof obj === "object" ||
                typeof obj === "function") &&
            isPublishEvent(obj.publish) as boolean ||
            (obj !== null &&
                typeof obj === "object" ||
                typeof obj === "function") &&
            isTransferObjectEvent(obj.transferObject) as boolean ||
            (obj !== null &&
                typeof obj === "object" ||
                typeof obj === "function") &&
            isDeleteObjectEvent(obj.deleteObject) as boolean ||
            (obj !== null &&
                typeof obj === "object" ||
                typeof obj === "function") &&
            isNewObjectEvent(obj.newObject) as boolean ||
            (obj !== null &&
                typeof obj === "object" ||
                typeof obj === "function") &&
            typeof obj.epochChange === "bigint" ||
            (obj !== null &&
                typeof obj === "object" ||
                typeof obj === "function") &&
            typeof obj.checkpoint === "bigint")
    )
}

export function isTransferObject(obj: any, _argumentName?: string): obj is TransferObject {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        isTransactionDigest(obj.recipient) as boolean &&
        isSuiObjectRef(obj.objectRef) as boolean
    )
}

export function isSuiTransferSui(obj: any, _argumentName?: string): obj is SuiTransferSui {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        isTransactionDigest(obj.recipient) as boolean &&
        (obj.amount === null ||
            isSuiMoveTypeParameterIndex(obj.amount) as boolean)
    )
}

export function isSuiChangeEpoch(obj: any, _argumentName?: string): obj is SuiChangeEpoch {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        isSuiMoveTypeParameterIndex(obj.epoch) as boolean &&
        isSuiMoveTypeParameterIndex(obj.storage_charge) as boolean &&
        isSuiMoveTypeParameterIndex(obj.computation_charge) as boolean
    )
}

export function isTransactionKindName(obj: any, _argumentName?: string): obj is TransactionKindName {
    return (
        (obj === "TransferObject" ||
            obj === "Publish" ||
            obj === "Call" ||
            obj === "TransferSui" ||
            obj === "ChangeEpoch")
    )
}

export function isSuiTransactionKind(obj: any, _argumentName?: string): obj is SuiTransactionKind {
    return (
        ((obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
            isTransferObject(obj.TransferObject) as boolean ||
            (obj !== null &&
                typeof obj === "object" ||
                typeof obj === "function") &&
            isSuiMovePackage(obj.Publish) as boolean ||
            (obj !== null &&
                typeof obj === "object" ||
                typeof obj === "function") &&
            isMoveCall(obj.Call) as boolean ||
            (obj !== null &&
                typeof obj === "object" ||
                typeof obj === "function") &&
            isSuiTransferSui(obj.TransferSui) as boolean ||
            (obj !== null &&
                typeof obj === "object" ||
                typeof obj === "function") &&
            isSuiChangeEpoch(obj.ChangeEpoch) as boolean)
    )
}

export function isSuiTransactionData(obj: any, _argumentName?: string): obj is SuiTransactionData {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        Array.isArray(obj.transactions) &&
        obj.transactions.every((e: any) =>
            isSuiTransactionKind(e) as boolean
        ) &&
        isTransactionDigest(obj.sender) as boolean &&
        isSuiObjectRef(obj.gasPayment) as boolean &&
        isSuiMoveTypeParameterIndex(obj.gasBudget) as boolean
    )
}

export function isEpochId(obj: any, _argumentName?: string): obj is EpochId {
    return (
        typeof obj === "number"
    )
}

export function isAuthorityQuorumSignInfo(obj: any, _argumentName?: string): obj is AuthorityQuorumSignInfo {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        isSuiMoveTypeParameterIndex(obj.epoch) as boolean &&
        Array.isArray(obj.signature) &&
        obj.signature.every((e: any) =>
            isEmptySignInfo(e) as boolean
        )
    )
}

export function isCertifiedTransaction(obj: any, _argumentName?: string): obj is CertifiedTransaction {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        isTransactionDigest(obj.transactionDigest) as boolean &&
        isSuiTransactionData(obj.data) as boolean &&
        isTransactionDigest(obj.txSignature) as boolean &&
        isAuthorityQuorumSignInfo(obj.authSignInfo) as boolean
    )
}

export function isGasCostSummary(obj: any, _argumentName?: string): obj is GasCostSummary {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        isSuiMoveTypeParameterIndex(obj.computationCost) as boolean &&
        isSuiMoveTypeParameterIndex(obj.storageCost) as boolean &&
        isSuiMoveTypeParameterIndex(obj.storageRebate) as boolean
    )
}

export function isExecutionStatusType(obj: any, _argumentName?: string): obj is ExecutionStatusType {
    return (
        (obj === "success" ||
            obj === "failure")
    )
}

export function isExecutionStatus(obj: any, _argumentName?: string): obj is ExecutionStatus {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        isExecutionStatusType(obj.status) as boolean &&
        (typeof obj.error === "undefined" ||
            isTransactionDigest(obj.error) as boolean)
    )
}

export function isOwnedObjectRef(obj: any, _argumentName?: string): obj is OwnedObjectRef {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        isObjectOwner(obj.owner) as boolean &&
        isSuiObjectRef(obj.reference) as boolean
    )
}

export function isTransactionEffects(obj: any, _argumentName?: string): obj is TransactionEffects {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        isExecutionStatus(obj.status) as boolean &&
        isGasCostSummary(obj.gasUsed) as boolean &&
        (typeof obj.sharedObjects === "undefined" ||
            Array.isArray(obj.sharedObjects) &&
            obj.sharedObjects.every((e: any) =>
                isSuiObjectRef(e) as boolean
            )) &&
        isTransactionDigest(obj.transactionDigest) as boolean &&
        (typeof obj.created === "undefined" ||
            Array.isArray(obj.created) &&
            obj.created.every((e: any) =>
                isOwnedObjectRef(e) as boolean
            )) &&
        (typeof obj.mutated === "undefined" ||
            Array.isArray(obj.mutated) &&
            obj.mutated.every((e: any) =>
                isOwnedObjectRef(e) as boolean
            )) &&
        (typeof obj.unwrapped === "undefined" ||
            Array.isArray(obj.unwrapped) &&
            obj.unwrapped.every((e: any) =>
                isOwnedObjectRef(e) as boolean
            )) &&
        (typeof obj.deleted === "undefined" ||
            Array.isArray(obj.deleted) &&
            obj.deleted.every((e: any) =>
                isSuiObjectRef(e) as boolean
            )) &&
        (typeof obj.wrapped === "undefined" ||
            Array.isArray(obj.wrapped) &&
            obj.wrapped.every((e: any) =>
                isSuiObjectRef(e) as boolean
            )) &&
        isOwnedObjectRef(obj.gasObject) as boolean &&
        (typeof obj.events === "undefined" ||
            Array.isArray(obj.events)) &&
        (typeof obj.dependencies === "undefined" ||
            Array.isArray(obj.dependencies) &&
            obj.dependencies.every((e: any) =>
                isTransactionDigest(e) as boolean
            ))
    )
}

export function isSuiTransactionResponse(obj: any, _argumentName?: string): obj is SuiTransactionResponse {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        isCertifiedTransaction(obj.certificate) as boolean &&
        isTransactionEffects(obj.effects) as boolean &&
        (obj.timestamp_ms === null ||
            isSuiMoveTypeParameterIndex(obj.timestamp_ms) as boolean) &&
        (obj.parsed_data === null ||
            (obj.parsed_data !== null &&
                typeof obj.parsed_data === "object" ||
                typeof obj.parsed_data === "function") &&
            isSuiParsedSplitCoinResponse(obj.parsed_data.SplitCoin) as boolean ||
            (obj.parsed_data !== null &&
                typeof obj.parsed_data === "object" ||
                typeof obj.parsed_data === "function") &&
            isSuiParsedMergeCoinResponse(obj.parsed_data.MergeCoin) as boolean ||
            (obj.parsed_data !== null &&
                typeof obj.parsed_data === "object" ||
                typeof obj.parsed_data === "function") &&
            isSuiParsedPublishResponse(obj.parsed_data.Publish) as boolean)
    )
}

export function isGatewayTxSeqNumber(obj: any, _argumentName?: string): obj is GatewayTxSeqNumber {
    return (
        typeof obj === "number"
    )
}

export function isGetTxnDigestsResponse(obj: any, _argumentName?: string): obj is GetTxnDigestsResponse {
    return (
        Array.isArray(obj) &&
        obj.every((e: any) =>
            Array.isArray(e) &&
            isSuiMoveTypeParameterIndex(e[0]) as boolean &&
            isTransactionDigest(e[1]) as boolean
        )
    )
}

export function isMoveCall(obj: any, _argumentName?: string): obj is MoveCall {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        isSuiObjectRef(obj.package) as boolean &&
        isTransactionDigest(obj.module) as boolean &&
        isTransactionDigest(obj.function) as boolean &&
        (typeof obj.typeArguments === "undefined" ||
            Array.isArray(obj.typeArguments) &&
            obj.typeArguments.every((e: any) =>
                isTransactionDigest(e) as boolean
            )) &&
        (typeof obj.arguments === "undefined" ||
            Array.isArray(obj.arguments) &&
            obj.arguments.every((e: any) =>
                isSuiJsonValue(e) as boolean
            ))
    )
}

export function isSuiJsonValue(obj: any, _argumentName?: string): obj is SuiJsonValue {
    return (
        (isTransactionDigest(obj) as boolean ||
            isSuiMoveTypeParameterIndex(obj) as boolean ||
            obj === false ||
            obj === true ||
            Array.isArray(obj) &&
            obj.every((e: any) =>
            (isTransactionDigest(e) as boolean ||
                isSuiMoveTypeParameterIndex(e) as boolean ||
                e === false ||
                e === true)
            ))
    )
}

export function isEmptySignInfo(obj: any, _argumentName?: string): obj is EmptySignInfo {
    return (
        typeof obj === "object"
    )
}

export function isAuthorityName(obj: any, _argumentName?: string): obj is AuthorityName {
    return (
        typeof obj === "string"
    )
}

export function isAuthoritySignature(obj: any, _argumentName?: string): obj is AuthoritySignature {
    return (
        typeof obj === "object"
    )
}

export function isTransactionBytes(obj: any, _argumentName?: string): obj is TransactionBytes {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        isTransactionDigest(obj.txBytes) as boolean &&
        isSuiObjectRef(obj.gas) as boolean
    )
}

export function isSuiParsedMergeCoinResponse(obj: any, _argumentName?: string): obj is SuiParsedMergeCoinResponse {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        isSuiObject(obj.updatedCoin) as boolean &&
        isSuiObject(obj.updatedGas) as boolean
    )
}

export function isSuiParsedSplitCoinResponse(obj: any, _argumentName?: string): obj is SuiParsedSplitCoinResponse {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        isSuiObject(obj.updatedCoin) as boolean &&
        Array.isArray(obj.newCoins) &&
        obj.newCoins.every((e: any) =>
            isSuiObject(e) as boolean
        ) &&
        isSuiObject(obj.updatedGas) as boolean
    )
}

export function isSuiParsedPublishResponse(obj: any, _argumentName?: string): obj is SuiParsedPublishResponse {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        Array.isArray(obj.createdObjects) &&
        obj.createdObjects.every((e: any) =>
            isSuiObject(e) as boolean
        ) &&
        isSuiPackage(obj.package) as boolean &&
        isSuiObject(obj.updatedGas) as boolean
    )
}

export function isSuiPackage(obj: any, _argumentName?: string): obj is SuiPackage {
    return (
        (obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
        isTransactionDigest(obj.digest) as boolean &&
        isTransactionDigest(obj.objectId) as boolean &&
        isSuiMoveTypeParameterIndex(obj.version) as boolean
    )
}

export function isSuiParsedTransactionResponse(obj: any, _argumentName?: string): obj is SuiParsedTransactionResponse {
    return (
        ((obj !== null &&
            typeof obj === "object" ||
            typeof obj === "function") &&
            isSuiParsedSplitCoinResponse(obj.SplitCoin) as boolean ||
            (obj !== null &&
                typeof obj === "object" ||
                typeof obj === "function") &&
            isSuiParsedMergeCoinResponse(obj.MergeCoin) as boolean ||
            (obj !== null &&
                typeof obj === "object" ||
                typeof obj === "function") &&
            isSuiParsedPublishResponse(obj.Publish) as boolean)
    )
}
