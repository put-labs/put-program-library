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
import {AuthorityType, createSetAuthorityInstruction} from '../src/generated'

/**
 * Create and initialize a new mint
 *
 * @param connection        Connection to use
 * @param ownerAccount      the owner account of the account that authority will be set
 * @param authorityType     set authority type,value takes 'freeze'、'mint'、'close'
 * @param authorizeAccount  the account that authority will be set
 * @param newAuthorize      the newAuthorize, if null authority will be set to None
 * @param confirmOptions  Options for confirming the transaction
 *
 * @return Address of the new mint
 */
export async function setAuthority(
    connection: Connection,
    ownerAccount: Signer,
    authorityType: string,
    authorizeAccount: PublicKey,
    newAuthorize? : PublicKey | null,
    confirmOptions?: ConfirmOptions,
) {
    log.info("set {:?} Authority", authorityType)
    const transaction = new Transaction().add(createSetAuthorityInstruction(
        {authorizeAccount: authorizeAccount, ownerAccount: ownerAccount.publicKey},
        {
            setAuthorityArgs: {
                authorityType: AuthorityType.FreezeAccount,
                newAuthority: newAuthorize
            }}
    ));
    log.info("come to here")
    const signature = await sendAndConfirmTransaction(connection, transaction, [ownerAccount], confirmOptions);
    log.info("signature", signature)
}
