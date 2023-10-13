import {
    ConfirmOptions,
    Connection,
    Keypair,
    PublicKey,
    sendAndConfirmTransaction,
    Signer,
    SystemProgram,
    Transaction,
} from '@com.put/web3.js';
import { TOKEN_PROGRAM_ID } from '../constants.js';
import { createInitializeMintInstruction } from '../instructions/initializeMint.js';
import { createMintMetaAccountInstruction } from '../instructions/createMintMetaAccount.js';
import { initializeMintMetaAccountInstruction } from '../instructions/initializeMintMetaAccount.js';
import { getMinimumBalanceForRentExemptMint, MINT_SIZE } from '../state/mint.js';

/**
 * Create and initialize a new mint
 *
 * @param connection      Connection to use
 * @param payer           Payer of the transaction and initialization fees
 * @param mintAuthority   Account or multisig that will control minting
 * @param freezeAuthority Optional account or multisig that can freeze token accounts
 * @param decimals        Location of the decimal place
 * @param keypair         Optional keypair, defaulting to a new random one
 * @param confirmOptions  Options for confirming the transaction
 * @param programId       SPL Token program account
 *
 * @return Address of the new mint
 */
export async function createMint(
    connection: Connection,
    payer: Signer,
    symbol: String,
    name: String,
    icon: String,
    mintAuthority: PublicKey,
    freezeAuthority: PublicKey | null,
    decimals: number,
    keypair = Keypair.generate(),
    confirmOptions?: ConfirmOptions,
    programId = TOKEN_PROGRAM_ID
): Promise<PublicKey> {
    const lamports = await getMinimumBalanceForRentExemptMint(connection);

    const [mintMeta,_] = await PublicKey.findProgramAddress(
        [ new TextEncoder().encode("MintMeta"), keypair.publicKey.toBuffer()],
        programId
    );

    const transaction = new Transaction().add(
        SystemProgram.createAccount({
            fromPubkey: payer.publicKey,
            newAccountPubkey: keypair.publicKey,
            space: MINT_SIZE,
            lamports,
            programId,
        }),
        createMintMetaAccountInstruction(payer.publicKey,keypair.publicKey,mintMeta,programId),
        createInitializeMintInstruction(keypair.publicKey, decimals, mintAuthority, freezeAuthority, programId),
        initializeMintMetaAccountInstruction(mintAuthority,keypair.publicKey,mintMeta,symbol,name,icon)
    );

    await sendAndConfirmTransaction(connection, transaction, [payer, keypair], confirmOptions);

    return keypair.publicKey;
}
