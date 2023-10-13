import { struct, u32, u8,u16,blob,} from '@com.put/buffer-layout';
import { publicKey, u64,u128,bool } from '@com.put/buffer-layout-utils';
import { Commitment, Connection, PublicKey, AccountInfo } from '@com.put/web3.js';
import { NATIVE_MINT, NATIVE_MINT_METS, TOKEN_PROGRAM_ID } from '../constants.js';
import {
    TokenAccountNotFoundError,
    TokenInvalidAccountError,
    TokenInvalidAccountOwnerError,
    TokenInvalidAccountSizeError,
} from '../errors.js';
import { AccountType, ACCOUNT_TYPE_SIZE } from '../extensions/accountType.js';




export interface Mintmeta {
    /** Address of the account */
    address: PublicKey;
    authorityOption: PublicKey | null;
    symbol: String;
    name: String;
    icon: String;
}


/** Mint as stored by the program */
export interface RawMintmeta {
    status: boolean;
    authorityOption: number;
    authority: PublicKey;
    meta: Uint8Array;
}



/** Buffer layout for de/serializing a mint */
export const MintmetaLayout = struct<RawMintmeta>([
    bool('status'),
    u32('authorityOption'),
    publicKey('authority'),
    blob(168,'meta'),
]);

/** Byte length of a mint */
export const MINT_META_SIZE = MintmetaLayout.span;

/**
 * Retrieve information about a mint
 *
 * @param connection Connection to use
 * @param address    Mint account
 * @param commitment Desired level of commitment for querying the state
 * @param programId  SPL Token program account
 *
 * @return Mint information
 */
 export async function getMintMeta(
    connection: Connection,
    address: PublicKey,
    commitment?: Commitment,
    programId = TOKEN_PROGRAM_ID
): Promise<Mintmeta> {

    let [meta_address,_] = await PublicKey.findProgramAddress(
        [ new TextEncoder().encode("MintMeta"), address.toBuffer()],
        programId
    );

    if (address == NATIVE_MINT) {
        meta_address = NATIVE_MINT_METS;
    }

    const info = await connection.getAccountInfo(meta_address, commitment);
    if (!info) throw new TokenAccountNotFoundError();
    if (!info.owner.equals(programId)) throw new TokenInvalidAccountOwnerError();
    if (info.data.length < MINT_META_SIZE) throw new TokenInvalidAccountSizeError();

    const rawMint = MintmetaLayout.decode(info.data.slice(0, MINT_META_SIZE));

    let meta = new TextDecoder("utf-8").decode(rawMint.meta).split('\n');
    if (meta.length < 3) {
        new TokenInvalidAccountSizeError();
    }
    return {
        address,
        authorityOption: rawMint.authorityOption ? rawMint.authority : null,
        symbol:meta[0],
        name:meta[1],
        icon:meta[2],
    };
}

/** Get the minimum lamport balance for a mint to be rent exempt
 *
 * @param connection Connection to use
 * @param commitment Desired level of commitment for querying the state
 *
 * @return Amount of lamports required
 */
export async function getMinimumBalanceForRentExemptMintMeta(
    connection: Connection,
    commitment?: Commitment
): Promise<bigint> {
    return await connection.getMinimumBalanceForRentExemption(MINT_META_SIZE, commitment);
}