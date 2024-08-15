# Marketplace 2.0 Pallet

The Marketplace pallet provides a market to buy and sell Nfts from pallet-nft using Asks and Bids.

## Overview

Users who own NFTs have the ability to create 'Asks' to offer their items to potential buyers. Once an Ask is created, the respective item becomes locked and cannot be transferred until either the item is sold or the Ask is canceled.

In the event that a user desires an item that is not currently available for sale, they can initiate a 'Bid' by specifying the price they are willing to pay for the item. The amount pledged in the Bid is then locked from the user's balance.

Both buyer and seller must pay fees for the operations that take place in the marketplace. This fees must be approved by the `FeeSigner` role, this approval is done by appending the signature of the FeeSigner in the creation of orders. Then the fees are payed to the `PayoutAddress` account configured inside the pallet.

## Dispatchable Functions

-   `force_set_authority()`: Sets authority role which has owner rights, only callable by root origin.
-   `set_fee_signer_address()`: Allows authority account to set the account that signs fees.
-   `set_payout_address()`: Allows authority account to set the payout address.
-   `create_order()`: Create Ask or Bid Order on an specific NFT (collectionId, ItemId). If orders match the transaction is executed.
-   `cancel_order()`: Cancelation of Ask or Bid order.
