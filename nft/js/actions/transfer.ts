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
import {createTransferInstruction} from '../src/generated'

/**
 * Create and initialize a new mint
 *
 * @param connection      Connection to use
 * @param from            the nft old owner
 * @param to              the nft new owner
 * @param nftAddress      the nft address
 * @param confirmOptions  Options for confirming the transaction
 *
 * @return Address of the new mint
 */
export async function transfer(
    connection: Connection,
    from: Signer,
    to : PublicKey,
    nftAddress: PublicKey,
    confirmOptions?: ConfirmOptions,
) {

    log.info("transferring a NFT {} from {} to {}", nftAddress.toBase58(), from.publicKey.toBase58(), to.toBase58())
    const transaction = new Transaction().add(createTransferInstruction(
        {from: from.publicKey, to: to, nftPubkey: nftAddress}
    ));
    const signature = await sendAndConfirmTransaction(connection, transaction, [from], confirmOptions);
    log.info("signature", signature)
}
