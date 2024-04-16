use runtime_common::Block;
use sc_transaction_pool::BasicPool;
use sc_transaction_pool::ChainApi;
use sc_transaction_pool_api::error;
use futures::Future;

pub struct CustomPool<PoolApi> where PoolApi: ChainApi<Block = Block>{
	inner_pool: BasicPool<PoolApi, Block>,
}
