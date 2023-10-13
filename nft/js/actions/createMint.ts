import {
    ConfirmOptions,
    Connection,
    Keypair,
    PublicKey,
    sendAndConfirmTransaction,
    Signer,
    SystemProgram,
    Transaction,
} from '@put/web3.js';
import {RENT_PROGRAM_ID, SYSTEM_PROGRAM_ID} from "../cli/constants";
import log from "loglevel";
import {createInitializeMintInstruction} from '../src/generated'

/**
 * Create and initialize a new mint
 *
 * @param connection      Connection to use
 * @param payer           Payer of the transaction and initialization fees
 * @param mintAuthority   Account or multisig that will control minting
 * @param totalSupply     the mint total supply
 * @param name            the mint name
 * @param symbol          the mint symbol
 * @param iconUri         the mint iconUri
 * @param mint            the new mint keypair
 * @param confirmOptions  Options for confirming the transaction
 *
 * @return Address of the new mint
 */
export async function createMint(
    connection: Connection,
    payer: Signer,
    mintAuthority: PublicKey,
    totalSupply: number,
    name: string,
    symbol: string,
    iconUri: string,
    mint = Keypair.generate(),
    confirmOptions?: ConfirmOptions,
): Promise<PublicKey> {

    const transaction = new Transaction().add(createInitializeMintInstruction(
        {mint: mint.publicKey, mintAuthority: payer.publicKey, systemProgram: SYSTEM_PROGRAM_ID, rent: RENT_PROGRAM_ID},
        {initializeMintArgs:
                {
                    totalSupply: totalSupply,
                    mintAuthority: payer.publicKey,
                    freezeAuthority: null,
                    name: name,
                    symbol: symbol,
                    iconUri: iconUri,
                }
        }
    ));
    const signature = await sendAndConfirmTransaction(connection, transaction, [payer, mint], confirmOptions);
    log.info("signature", signature)

    return mint.publicKey;
}
