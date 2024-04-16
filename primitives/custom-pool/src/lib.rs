use sp_runtime::traits::Block as BlockT; 
use sc_transaction_pool::BasicPool;
use sc_transaction_pool::ChainApi;


pub struct CustomPool<PoolApi,Block> where Block: BlockT, PoolApi: ChainApi<Block = Block>{
	inner_pool: BasicPool<PoolApi, Block>,
}
