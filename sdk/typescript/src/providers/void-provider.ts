// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import { PublicKey, SignatureScheme } from '../cryptography/publickey';
import { HttpHeaders } from '../rpc/client';
import { Base64DataBuffer } from '../serialization/base64';
import { RawMoveCall } from '../signers/txn-data-serializers/txn-data-serializer';
import {
  CertifiedTransaction,
  TransactionDigest,
  GetTxnDigestsResponse,
  GatewayTxSeqNumber,
  SuiObjectInfo,
  GetObjectDataResponse,
  SuiObjectRef,
  SuiMoveFunctionArgTypes,
  SuiMoveNormalizedFunction,
  SuiMoveNormalizedStruct,
  SuiMoveNormalizedModule,
  SuiMoveNormalizedModules,
  SuiEventFilter,
  SuiEventEnvelope,
  SubscriptionId,
  ExecuteTransactionRequestType,
  SuiExecuteTransactionResponse,
  SuiAddress,
  ObjectId,
  TransactionQuery,
  PaginatedTransactionDigests,
  EventQuery,
  PaginatedEvents,
  EventId,
  RpcApiVersion,
  FaucetResponse,
  Order,
  TransactionEffects,
  CoinMetadata,
  DevInspectResults,
  PaginatedCoins,
  BalanceStruct,
  SupplyStruct,
} from '../types';
import { Provider } from './provider';

import { DynamicFieldPage } from '../types/dynamic_fields';

export class VoidProvider extends Provider {
  // API Version
  async getRpcApiVersion(): Promise<RpcApiVersion | undefined> {
    throw this.newError('getRpcApiVersion');
  }

  // Governance
  async getReferenceGasPrice(): Promise<number> {
    throw this.newError('getReferenceGasPrice');
  }

  // Faucet
  async requestSuiFromFaucet(
    _recipient: SuiAddress,
    _httpHeaders?: HttpHeaders
  ): Promise<FaucetResponse> {
    throw this.newError('requestSuiFromFaucet');
  }

  // RPC Endpoint
  call(
    _endpoint: string, 
    _params: any[]): Promise<any> {
    throw this.newError('call');
  }

  // Coins
  async getCoins(
    _owner: SuiAddress,
    _coinType: String | null,
    _cursor: ObjectId | null,
    _limit: number | null
  ) : Promise<PaginatedCoins> {
    throw this.newError('getCoins');
  }

  async getAllCoins(
    _owner: SuiAddress,
    _cursor: ObjectId | null,
    _limit: number | null
  ) : Promise<PaginatedCoins> {
    throw this.newError('getAllCoins');
  }

  async getBalance(
    _owner: string, 
    _coinType: String | null
    ): Promise<BalanceStruct> {
      throw this.newError('getBalance');
  }

  async getAllBalances(
    _owner: string, 
    ): Promise<BalanceStruct[]> {
      throw this.newError('getAllBalances');
  }

  async getCoinMetadata(_coinType: string): Promise<CoinMetadata> {
    throw new Error('getCoinMetadata');
  }

  async getTotalSupply(
    _coinType: string
  ) : Promise<SupplyStruct> {
    throw new Error('getTotalSupply');
  }

  // Objects
  async getObjectsOwnedByAddress(_address: string): Promise<SuiObjectInfo[]> {
    throw this.newError('getObjectsOwnedByAddress');
  }

  async getGasObjectsOwnedByAddress(
    _address: string
  ): Promise<SuiObjectInfo[]> {
    throw this.newError('getGasObjectsOwnedByAddress');
  }

  async getCoinBalancesOwnedByAddress(
    _address: string,
    _typeArg?: string
  ): Promise<GetObjectDataResponse[]> {
    throw this.newError('getCoinBalancesOwnedByAddress');
  }

  async selectCoinsWithBalanceGreaterThanOrEqual(
    _address: string,
    _amount: bigint,
    _typeArg: string,
    _exclude: ObjectId[] = []
  ): Promise<GetObjectDataResponse[]> {
    throw this.newError('selectCoinsWithBalanceGreaterThanOrEqual');
  }

  async selectCoinSetWithCombinedBalanceGreaterThanOrEqual(
    _address: string,
    _amount: bigint,
    _typeArg: string,
    _exclude: ObjectId[]
  ): Promise<GetObjectDataResponse[]> {
    throw this.newError('selectCoinSetWithCombinedBalanceGreaterThanOrEqual');
  }

  async getObject(_objectId: string): Promise<GetObjectDataResponse> {
    throw this.newError('getObject');
  }

  async getObjectRef(_objectId: string): Promise<SuiObjectRef | undefined> {
    throw this.newError('getObjectRef');
  }

  // Transactions
  async getTransaction(
    _digest: TransactionDigest
  ): Promise<CertifiedTransaction> {
    throw this.newError('getTransaction');
  }

  async executeTransaction(
    _txnBytes: Base64DataBuffer,
    _signatureScheme: SignatureScheme,
    _signature: Base64DataBuffer,
    _pubkey: PublicKey,
    _requestType: ExecuteTransactionRequestType
  ): Promise<SuiExecuteTransactionResponse> {
    throw this.newError('executeTransaction with request Type');
  }

  devInspectTransaction(_txBytes: string): Promise<DevInspectResults> {
    throw this.newError('devInspectTransaction');
  }

  async devInspectMoveCall(
    _sender: SuiAddress,
    _moveCall: RawMoveCall
  ): Promise<DevInspectResults> {
    throw this.newError('devInspectMoveCall');
  }

  dryRunTransaction(_txBytes: string): Promise<TransactionEffects> {
    throw this.newError('dryRunTransaction');
  }

  getDynamicFields(
    _parent_object_id: ObjectId,
    _cursor: ObjectId | null = null,
    _limit: number | null = null
  ): Promise<DynamicFieldPage> {
    throw this.newError('getDynamicFields');
  }

  getDynamicFieldObject(
    _parent_object_id: ObjectId,
    _name: string
  ): Promise<GetObjectDataResponse> {
    throw this.newError('getDynamicFieldObject');
  }

  async getTotalTransactionNumber(): Promise<number> {
    throw this.newError('getTotalTransactionNumber');
  }

  async getTransactionDigestsInRange(
    _start: GatewayTxSeqNumber,
    _end: GatewayTxSeqNumber
  ): Promise<GetTxnDigestsResponse> {
    throw this.newError('getTransactionDigestsInRange');
  }

  async getMoveFunctionArgTypes(
    _objectId: string,
    _moduleName: string,
    _functionName: string
  ): Promise<SuiMoveFunctionArgTypes> {
    throw this.newError('getMoveFunctionArgTypes');
  }

  async getNormalizedMoveModulesByPackage(
    _objectId: string
  ): Promise<SuiMoveNormalizedModules> {
    throw this.newError('getNormalizedMoveModulesByPackage');
  }

  async getNormalizedMoveModule(
    _objectId: string,
    _moduleName: string
  ): Promise<SuiMoveNormalizedModule> {
    throw this.newError('getNormalizedMoveModule');
  }

  async getNormalizedMoveFunction(
    _objectId: string,
    _moduleName: string,
    _functionName: string
  ): Promise<SuiMoveNormalizedFunction> {
    throw this.newError('getNormalizedMoveFunction');
  }

  async getNormalizedMoveStruct(
    _objectId: string,
    _oduleName: string,
    _structName: string
  ): Promise<SuiMoveNormalizedStruct> {
    throw this.newError('getNormalizedMoveStruct');
  }

  async syncAccountState(_address: string): Promise<any> {
    throw this.newError('syncAccountState');
  }

  async subscribeEvent(
    _filter: SuiEventFilter,
    _onMessage: (event: SuiEventEnvelope) => void
  ): Promise<SubscriptionId> {
    throw this.newError('subscribeEvent');
  }

  async unsubscribeEvent(_id: SubscriptionId): Promise<boolean> {
    throw this.newError('unsubscribeEvent');
  }

  private newError(operation: string): Error {
    return new Error(`Please use a valid provider for ${operation}`);
  }

  async getTransactions(
    _query: TransactionQuery,
    _cursor: TransactionDigest | null,
    _limit: number | null,
    _order: Order
  ): Promise<PaginatedTransactionDigests> {
    throw this.newError('getTransactions');
  }

  async getEvents(
    _query: EventQuery,
    _cursor: EventId | null,
    _limit: number | null,
    _order: Order
  ): Promise<PaginatedEvents> {
    throw this.newError('getEvents');
  }
}
