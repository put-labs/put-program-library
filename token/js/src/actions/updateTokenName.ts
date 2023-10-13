import {
    ConfirmOptions,
    Connection,
    PublicKey,
    sendAndConfirmTransaction,
    Signer,
    Transaction,
    TransactionSignature,
} from '@com.put/web3.js';
import { TOKEN_PROGRAM_ID } from '../constants.js';
import { createUpdateTokenNameInstruction } from '../instructions/updateTokenName.js';
import { getSigners } from './internal.js';

/**
 * updateTokenIcon for Mint tokens
 * @param connection     Connection to use
 * @param payer          Payer of the transaction fees
 * @param mint           Mint for the account
 * @param authority      Minting authority
 * @param name           Name to update
 * @param multiSigners   Signing accounts if `authority` is a multisig
 * @param confirmOptions Options for confirming the transaction
 * @param programId      SPL Token program account
 *
 * @return Signature of the confirmed transaction
 */
export async function updateTokenName(
    connection: Connection,
    payer: Signer,
    mint: PublicKey,
    authority: Signer | PublicKey,
    name: string,
    multiSigners: Signer[] = [],
    confirmOptions?: ConfirmOptions,
    programId = TOKEN_PROGRAM_ID
): Promise<TransactionSignature> {
    const [authorityPublicKey, signers] = getSigners(authority, multiSigners);

    const [mintMeta,_] = await PublicKey.findProgramAddress(
        [ new TextEncoder().encode("MintMeta"), mint.toBuffer()],
        programId
    );

    const transaction = new Transaction().add(
        createUpdateTokenNameInstruction( mintMeta, authorityPublicKey, name, programId)
    );

    return await sendAndConfirmTransaction(connection, transaction, [payer, ...signers], confirmOptions);
}
