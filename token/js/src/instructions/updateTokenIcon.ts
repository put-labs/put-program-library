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
export interface UpdateTokenIconInstructionData {
    instruction: TokenInstruction.UpdateIcon;
    icon: Uint8Array;
}

/** TODO: docs */
export const updateTokenIconInstructionData = struct<UpdateTokenIconInstructionData>([
    u8('instruction'),
    blob(128,'icon'),
]);

/**
 * Construct an createUpdateTokenIcon instruction
 *
 * @param mintMetaAuthority   Minting authority
 * @param icon            Token name
 * @param programId       SPL Token program account
 *
 * @return Instruction to add to a transaction
 */
export function createUpdateTokenIconInstruction(
    mintMeta: PublicKey,
    mintMetaAuthority: PublicKey,
    icon: String,
    programId = TOKEN_PROGRAM_ID
): TransactionInstruction {
    const keys = [
        {pubkey: mintMeta, isSigner: false, isWritable: true},
        {pubkey: mintMetaAuthority, isSigner: true, isWritable: false},
    ];

    const data = Buffer.alloc(updateTokenIconInstructionData.span);


    let icon_buff = Buffer.from((icon + '\n').toString());
    const icon_zeroPad = Buffer.alloc(128);
    icon_buff.copy(icon_zeroPad);

    updateTokenIconInstructionData.encode(
        {
            instruction: TokenInstruction.UpdateIcon,
            icon: icon_zeroPad,
        },
        data
    );

    return new TransactionInstruction({ keys, programId, data });
}

/** A decoded, valid UpdateTokenIcon instruction */
export interface DecodedUpdateTokenIconInstruction {
    programId: PublicKey;
    keys: {
        mintMeta: AccountMeta,
        mintMetaAuthority: AccountMeta;
    };
    data: {
        instruction: TokenInstruction.UpdateIcon;
        icon: String;
    };
}

/**
 * Decode an UpdateTokenIcon instruction and validate it
 *
 * @param instruction Transaction instruction to decode
 * @param programId   SPL Token program account
 *
 * @return Decoded, valid instruction
 */
export function decodeTokenIconInstruction(
    instruction: TransactionInstruction,
    programId = TOKEN_PROGRAM_ID
): DecodedUpdateTokenIconInstruction {
    if (!instruction.programId.equals(programId)) throw new TokenInvalidInstructionProgramError();
    if (instruction.data.length !== updateTokenIconInstructionData.span) throw new TokenInvalidInstructionDataError();

    const {
        keys: { mintMeta, mintMetaAuthority },
        data,
    } = decodeTokenIconInstructionUnchecked(instruction);
    if (data.instruction !== TokenInstruction.UpdateIcon) throw new TokenInvalidInstructionTypeError();
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

/** A decoded, non-validated UpdateTokenIcon instruction */
export interface DecodedTokenIconInstructionUnchecked {
    programId: PublicKey;
    keys: {
        mintMeta: AccountMeta | undefined;
        mintMetaAuthority: AccountMeta | undefined;
    };
    data: {
        instruction: number;
        icon: String;
    };
}

/**
 * Decode an UpdateTokenIcon instruction without validating it
 *
 * @param instruction Transaction instruction to decode
 *
 * @return Decoded, non-validated instruction
 */
export function decodeTokenIconInstructionUnchecked({
    programId,
    keys: [mintMeta, mintMetaAuthority],
    data,
}: TransactionInstruction): DecodedTokenIconInstructionUnchecked {
    const { instruction, icon } = updateTokenIconInstructionData.decode(data);

    let str_icon =  new TextDecoder("utf-8").decode(icon).split('\n')[0];

    return {
        programId,
        keys: {
            mintMeta,
            mintMetaAuthority,
        },
        data: {
            instruction,
            icon:str_icon,
        },
    };
}
