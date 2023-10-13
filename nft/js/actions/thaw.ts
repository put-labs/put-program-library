import {
    ConfirmOptions,
    Connection,
    PublicKey,
    sendAndConfirmTransaction,
    Signer,
    Transaction,
} from '@put/web3.js';
import log from "loglevel";
import {createThawInstruction} from '../src/generated'

/**
 * Create and initialize a new mint
 *
 * @param connection      Connection to use
 * @param sender          the mint freeze authority
 * @param mintAddress     the mint address
 * @param nftAddress      the nft address
 * @param confirmOptions  Options for confirming the transaction
 *
 * @return Address of the new mint
 */
export async function thaw(
    connection: Connection,
    sender: Signer,
    mintAddress: PublicKey,
    nftAddress: PublicKey,
    confirmOptions?: ConfirmOptions,
) {
    log.info("thawing a NFT {}", nftAddress.toBase58())
    const transaction = new Transaction().add(createThawInstruction(
        {authorityAccount: sender.publicKey, mintAccount: mintAddress, nftAccount: nftAddress}
    ));
    const signature = await sendAndConfirmTransaction(connection, transaction, [sender], confirmOptions);
    log.info("signature", signature)
}
