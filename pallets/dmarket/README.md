# Dmarket Pallet

The Dmarket pallet provides a marketplace for buying and selling NFTs that are managed by the `pallet-nfts`, based on the dmarket Smart contracts developed by Mythical.

## Overview

This project enables users to securely trade NFTs that are part of the Dmarket collection by allowing both the seller and buyer to agree on the terms and digitally sign a message approving the trade. The signed messages include specific parameters that ensure both parties are in agreement before the trade is executed on the blockchain.

For the seller:

-   Domain: The network domain identifier, specifying the environment in which the trade is executed. Helps to prevent transaction replay on other chains that use this very same pallet.
-   Sender: The account authorized to submit the trade transaction to the blockchain.
-   FeeAccount: The account designated to receive the trade fee.
-   ItemId: The unique identifier of the NFT being traded. Must be part of the Dmarket Colection
-   Price: The selling price set by the seller for the NFT.
-   AskExpirationAt: The expiration timestamp, after which the seller's signature is no longer valid.

For the Buyer:

-   Domain, Sender, FeeAccount, ItemId, and Price: These parameters must match those in the seller's message to ensure both parties are in agreement.

-   Fee: The amount of tokens the buyer agrees to pay as a fee for the trade.
-   BiExpirationAt: The expiration timestamp, after which the buyer's signature is no longer valid.

Once the seller and buyer have signed their respective messages, an agreed-upon sender can submit the trade to the blockchain. The transaction validates the signatures against the provided trade parameters. If the signatures are valid and the trade conditions are met, the NFT is transferred from the seller to the buyer. Simultaneously, the agreed-upon price is transferred from the buyer to the seller, and the fee is transferred to the FeeAccount.

## Dispatchable Functions

-   `force_set_collection()`: Sets the Dmarket collection. Only callable by root.
-   `execute_trade()`: Execute a trade between a seller and a buyer for a specific NFT (item) in the configured DmarketCollection. Callable by anyone as long as the origin matches the sender field inside both Ask and Bid signed messages.
