import { blob, struct, u16, u8 } from '@com.put/buffer-layout';
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
export interface UpdateTokenSymbolInstructionData {
    instruction: TokenInstruction.UpdateSymbol;
    symbol: Uint8Array;
}

/** TODO: docs */
export const updateTokenSymbolInstructionData = struct<UpdateTokenSymbolInstructionData>([
    u8('instruction'),
    blob(128,'symbol'),
]);

/**
 * Construct an createUpdateTokenSymbol instruction
 *
 * @param mint            Token mint account
 * @param mintMetaAuthority   Minting authority
 * @param symbol            Token name
 * @param programId       SPL Token program account
 *
 * @return Instruction to add to a transaction
 */
export function createUpdateTokenSymbolInstruction(
    mintMeta: PublicKey,
    mintMetaAuthority: PublicKey,
    symbol: String,
    programId = TOKEN_PROGRAM_ID
): TransactionInstruction {
    const keys = [
        {pubkey: mintMeta, isSigner: false, isWritable: true},
        {pubkey: mintMetaAuthority, isSigner: true, isWritable: false},
    ];

    const data = Buffer.alloc(updateTokenSymbolInstructionData.span);


    let symbol_buff = Buffer.from((symbol + '\n').toString());
    const symbol_zeroPad = Buffer.alloc(128);
    symbol_buff.copy(symbol_zeroPad);

    updateTokenSymbolInstructionData.encode(
        {
            instruction: TokenInstruction.UpdateSymbol,
            symbol: symbol_zeroPad,
        },
        data
    );

    return new TransactionInstruction({ keys, programId, data });
}

/** A decoded, valid UpdateTokenSymbol instruction */
export interface DecodedUpdateTokenSymbolInstruction {
    programId: PublicKey;
    keys: {
        mintMeta: AccountMeta,
        mintMetaAuthority: AccountMeta;
    };
    data: {
        instruction: TokenInstruction.UpdateSymbol;
        symbol: String;
    };
}

/**
 * Decode an UpdateTokenSymbol instruction and validate it
 *
 * @param instruction Transaction instruction to decode
 * @param programId   SPL Token program account
 *
 * @return Decoded, valid instruction
 */
export function decodeTokenSymbolInstruction(
    instruction: TransactionInstruction,
    programId = TOKEN_PROGRAM_ID
): DecodedUpdateTokenSymbolInstruction {
    if (!instruction.programId.equals(programId)) throw new TokenInvalidInstructionProgramError();
    if (instruction.data.length !== updateTokenSymbolInstructionData.span) throw new TokenInvalidInstructionDataError();

    const {
        keys: { mintMeta, mintMetaAuthority },
        data,
    } = decodeTokenSymbolInstructionUnchecked(instruction);
    if (data.instruction !== TokenInstruction.UpdateSymbol) throw new TokenInvalidInstructionTypeError();
    if (!mintMeta || !mintMetaAuthority) throw new TokenInvalidInstructionKeysError();

    // TODO: key checks?

    return {
        programId,
        keys: {
            mintMeta,
            mintMetaAuthority,
        },
        data,
    };
}

/** A decoded, non-validated UpdateTokenSymbol instruction */
export interface DecodedTokenSymbolInstructionUnchecked {
    programId: PublicKey;
    keys: {
        mintMeta: AccountMeta | undefined;
        mintMetaAuthority: AccountMeta | undefined;
    };
    data: {
        instruction: number;
        symbol: String;
    };
}

/**
 * Decode an UpdateTokenSymbol instruction without validating it
 *
 * @param instruction Transaction instruction to decode
 *
 * @return Decoded, non-validated instruction
 */
export function decodeTokenSymbolInstructionUnchecked({
    programId,
    keys: [mintMeta, mintMetaAuthority],
    data,
}: TransactionInstruction): DecodedTokenSymbolInstructionUnchecked {
    const { instruction, symbol } = updateTokenSymbolInstructionData.decode(data);

    let str_symbol =  new TextDecoder("utf-8").decode(symbol).split('\n')[0];

    return {
        programId,
        keys: {
            mintMeta,
            mintMetaAuthority,
        },
        data: {
            instruction,
            symbol:str_symbol,
        },
    };
}
