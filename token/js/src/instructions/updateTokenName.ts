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
export interface UpdateTokenNameInstructionData {
    instruction: TokenInstruction.UpdateName;
    name: Uint8Array;
}

/** TODO: docs */
export const updateTokenNameInstructionData = struct<UpdateTokenNameInstructionData>([
    u8('instruction'),
    blob(128,'name'),
]);

/**
 * Construct an createUpdateTokenName instruction
 *
 * @param mintMetaAuthority   Minting authority
 * @param icon            Token name
 * @param programId       SPL Token program account
 *
 * @return Instruction to add to a transaction
 */
export function createUpdateTokenNameInstruction(
    mintMeta: PublicKey,
    mintMetaAuthority: PublicKey,
    name: String,
    programId = TOKEN_PROGRAM_ID
): TransactionInstruction {
    const keys = [
        {pubkey: mintMeta, isSigner: false, isWritable: true},
        {pubkey: mintMetaAuthority, isSigner: true, isWritable: false},
    ];

    const data = Buffer.alloc(updateTokenNameInstructionData.span);


    let name_buff = Buffer.from((name + '\n').toString());
    const name_zeroPad = Buffer.alloc(128);
    name_buff.copy(name_zeroPad);

    updateTokenNameInstructionData.encode(
        {
            instruction: TokenInstruction.UpdateName,
            name: name_zeroPad,
        },
        data
    );

    return new TransactionInstruction({ keys, programId, data });
}

/** A decoded, valid UpdateTokenName instruction */
export interface DecodedUpdateNameInstruction {
    programId: PublicKey;
    keys: {
        mintMeta: AccountMeta,
        mintMetaAuthority: AccountMeta;
    };
    data: {
        instruction: TokenInstruction.UpdateName;
        name: String;
    };
}

/**
 * Decode an UpdateTokenName instruction and validate it
 *
 * @param instruction Transaction instruction to decode
 * @param programId   SPL Token program account
 *
 * @return Decoded, valid instruction
 */
export function decodeUpdateTokenNameInstruction(
    instruction: TransactionInstruction,
    programId = TOKEN_PROGRAM_ID
): DecodedUpdateNameInstruction {
    if (!instruction.programId.equals(programId)) throw new TokenInvalidInstructionProgramError();
    if (instruction.data.length !== updateTokenNameInstructionData.span) throw new TokenInvalidInstructionDataError();

    const {
        keys: { mintMeta, mintMetaAuthority },
        data,
    } = decodeUpdateTokenNameInstructionUnchecked(instruction);
    if (data.instruction !== TokenInstruction.UpdateName) throw new TokenInvalidInstructionTypeError();
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

/** A decoded, non-validated UpdateTokenName instruction */
export interface DecodedUpdateTokenNameInstructionUnchecked {
    programId: PublicKey;
    keys: {
        mintMeta: AccountMeta | undefined;
        mintMetaAuthority: AccountMeta | undefined;
    };
    data: {
        instruction: number;
        name: String;
    };
}

/**
 * Decode an UpdateTokenName instruction without validating it
 *
 * @param instruction Transaction instruction to decode
 *
 * @return Decoded, non-validated instruction
 */
export function decodeUpdateTokenNameInstructionUnchecked({
    programId,
    keys: [mintMeta, mintMetaAuthority],
    data,
}: TransactionInstruction): DecodedUpdateTokenNameInstructionUnchecked {
    const { instruction, name } = updateTokenNameInstructionData.decode(data);

    let str_name =  new TextDecoder("utf-8").decode(name).split('\n')[0];

    return {
        programId,
        keys: {
            mintMeta,
            mintMetaAuthority,
        },
        data: {
            instruction,
            name: str_name,
        },
    };
}
