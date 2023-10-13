import {
    ConfirmOptions,
    Connection,
    Keypair,
    PublicKey,
    sendAndConfirmTransaction,
    Signer,
    Transaction,
} from '@put/web3.js';
import log from "loglevel";
import {createBurnInstruction} from '../src/generated'

/**
 * Create and initialize a new mint
 *
 * @param connection      Connection to use
 * @param sender          the nft owner
 * @param nftAddress      the nft address
 * @param confirmOptions  Options for confirming the transaction
 *
 * @return Address of the new mint
 */
export async function burn(
    connection: Connection,
    sender: Signer,
    nftAddress: PublicKey,
    confirmOptions?: ConfirmOptions,
) {
    log.info("burning a NFT {}", nftAddress.toBase58())
    const transaction = new Transaction().add(createBurnInstruction(
        {authorityAccount: sender.publicKey, nftAccount: nftAddress}
    ));
    const signature = await sendAndConfirmTransaction(connection, transaction, [sender], confirmOptions);
    log.info("signature", signature)
}
