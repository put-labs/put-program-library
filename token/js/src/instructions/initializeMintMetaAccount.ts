import { blob, struct, u16, u8, utf8 } from '@com.put/buffer-layout';
import { publicKey } from '@com.put/buffer-layout-utils';
import { AccountMeta, PublicKey, SYSVAR_RENT_PUBKEY, TransactionInstruction } from '@com.put/web3.js';
import { TOKEN_PROGRAM_ID } from '../constants.js';
import {
    TokenInvalidInstructionDataError,
    TokenInvalidInstructionKeysError,
    TokenInvalidInstructionProgramError,
    TokenInvalidInstructionTypeError,
} from '../errors.js';
import { TokenInstruction } from './types.js';

/** TODO: docs */
export interface InitializeMintMetaAccountInstructionData {
    instruction: TokenInstruction.InitMintMetaAccount;
    meta: Uint8Array;
}

/** TODO: docs */
export const initializeMintMetaAccountInstructionData = struct<InitializeMintMetaAccountInstructionData>([
    u8('instruction'),
    blob(168,'meta'),
]);

/**
 * Construct an InitializeMintMetaAccount instruction
 *
 * @param mintAuthority   MintMetaAccounting authority
 * @param mint            Token mint account
 * @param mintMeta            Token mint account
 * @param symbol          Token symbol
 * @param name            Token name
 * @param icon            Token icon
 * @param programId       SPL Token program account
 *
 * @return Instruction to add to a transaction
 */
export function initializeMintMetaAccountInstruction(
    mintAuthority: PublicKey,
    mint: PublicKey,
    mintMeta: PublicKey,
    symbol: String,
    name: String,
    icon: String,
    programId = TOKEN_PROGRAM_ID
): TransactionInstruction {
    const keys = [
        { pubkey: mint, isSigner: false, isWritable: false },
        { pubkey: mintMeta, isSigner: false, isWritable: true },
    ];

    const data = Buffer.alloc(initializeMintMetaAccountInstructionData.span);

    let meta_str = symbol + '\n' + name + '\n' + icon + '\n';
    let meta_buff = Buffer.from(meta_str.toString());
    let buff_zeroPad = Buffer.alloc(168);
    meta_buff.copy(buff_zeroPad);

    initializeMintMetaAccountInstructionData.encode(
        {
            instruction: TokenInstruction.InitMintMetaAccount,
            meta : buff_zeroPad,
        },
        data
    );

    return new TransactionInstruction({ keys, programId, data });
}

/** A decoded, valid InitializeMintMetaAccount instruction */
export interface DecodedInitializeMintMetaAccountInstruction {
    programId: PublicKey;
    keys: {
        mint: AccountMeta;
        mintMeta: AccountMeta;
    };
    data: {
        instruction: TokenInstruction.InitMintMetaAccount;
        meta: String;
    };
}

/**
 * Decode an InitializeMintMetaAccount instruction and validate it
 *
 * @param instruction Transaction instruction to decode
 * @param programId   SPL Token program account
 *
 * @return Decoded, valid instruction
 */
export function decodeInitializeMintMetaAccountInstruction(
    instruction: TransactionInstruction,
    programId = TOKEN_PROGRAM_ID
): DecodedInitializeMintMetaAccountInstruction {
    if (!instruction.programId.equals(programId)) throw new TokenInvalidInstructionProgramError();
    if (instruction.data.length !== initializeMintMetaAccountInstructionData.span) throw new TokenInvalidInstructionDataError();

    const {
        keys: { mint, mintMeta},
        data,
    } = decodeInitializeMintMetaAccountInstructionUnchecked(instruction);
    if (data.instruction !== TokenInstruction.InitMintMetaAccount) throw new TokenInvalidInstructionTypeError();
    if ( !mint || !mintMeta) throw new TokenInvalidInstructionKeysError();

    // TODO: key checks?

    return {
        programId,
        keys: {
            mint,
            mintMeta,
        },
        data,
    };
}

/** A decoded, non-validated InitializeMintMetaAccount instruction */
export interface DecodedInitializeMintMetaAccountInstructionUnchecked {
    programId: PublicKey;
    keys: {
        mint: AccountMeta | undefined;
        mintMeta: AccountMeta | undefined;
    };
    data: {
        instruction: number;
        meta: String;
    };
}

/**
 * Decode an InitializeMintMetaAccount instruction without validating it
 *
 * @param instruction Transaction instruction to decode
 *
 * @return Decoded, non-validated instruction
 */
export function decodeInitializeMintMetaAccountInstructionUnchecked({
    programId,
    keys: [mint, mintMeta],
    data,
}: TransactionInstruction): DecodedInitializeMintMetaAccountInstructionUnchecked {
    const { instruction, meta } =
        initializeMintMetaAccountInstructionData.decode(data);

    return {
        programId,
        keys: {
            mint,
            mintMeta,
        },
        data: {
            instruction,
            meta:new TextDecoder("utf-8").decode(meta),
        },
    };
}
