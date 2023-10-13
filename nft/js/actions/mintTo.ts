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
import {PROGRAM_ID, RENT_PROGRAM_ID, SYSTEM_PROGRAM_ID} from "../cli/constants";
import log from "loglevel";
import {createMintToInstruction} from '../src/generated'
import * as buffer from "buffer";
import {min} from "bn.js";
const BN = require("bn.js").BN;

/**
 * Create and initialize a new mint
 *
 * @param connection      Connection to use
 * @param payer           Payer of the transaction and initialization fees
 * @param mint            the mint pubkey
 * @param tokenUri        the nft uri
 * @param confirmOptions  Options for confirming the transaction
 *
 * @return Address of the new mint
 */
export async function mintTo(
    connection: Connection,
    payer: Signer,
    mint : PublicKey,
    tokenUri: string,
    confirmOptions?: ConfirmOptions,
): Promise<PublicKey> {
    const info = await connection.getAccountInfo(mint, "finalized");

    const supply = buffer.Buffer.from(info.data.slice(32, 32 + 8));


    let bnSupply = new BN(supply, "le")
    let index = bnSupply.add(new BN(1));
    let indexBn = new BN(index, 'le');
    const indexBuffer = indexBn.toArray('le', 8);

    const programIdBuffer = PROGRAM_ID.toBuffer()
    const mintIdBuffer = mint.toBuffer();
    // const seeds = Buffer.concat([indexBuffer, programIdBuffer, mintIdBuffer])
    const nftPubkey = PublicKey.findProgramAddressSync([indexBuffer, programIdBuffer, mintIdBuffer], PROGRAM_ID)
    log.info("minting a new NFT:", nftPubkey[0].toBase58())
    const transaction = new Transaction().add(createMintToInstruction(
        { nftPubkey: nftPubkey[0],mint: mint, owner: payer.publicKey, systemProgram: SYSTEM_PROGRAM_ID, rent: RENT_PROGRAM_ID},
        {instructionArgs: tokenUri}
    ));
    const signature = await sendAndConfirmTransaction(connection, transaction, [payer], confirmOptions);
    log.info("signature", signature)

    return nftPubkey[0];
}
