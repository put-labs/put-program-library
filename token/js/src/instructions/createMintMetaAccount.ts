import { blob, struct, u16, u8 } from '@com.put/buffer-layout';
import { publicKey } from '@com.put/buffer-layout-utils';
import { AccountMeta, PublicKey, SYSVAR_RENT_PUBKEY, TransactionInstruction } from '@com.put/web3.js';
import { TOKEN_PROGRAM_ID , SYSTEM_PROGRAM_ID } from '../constants.js';
import {
    TokenInvalidInstructionDataError,
    TokenInvalidInstructionKeysError,
    TokenInvalidInstructionProgramError,
    TokenInvalidInstructionTypeError,
} from '../errors.js';
import { TokenInstruction } from './types.js';

/** TODO: docs */
export interface CreateMintMetaAccountInstructionData {
    instruction: TokenInstruction.CreateMintMetaAccount;
}

/** TODO: docs */
export const createMintMetaAccountInstructionData = struct<CreateMintMetaAccountInstructionData>([
    u8('instruction'),
]);

/**
 * Construct an createCreateMintMetaAccount instruction
 *
 * @param payer             Token payer account
 * @param mint              Token mint account
 * @param mintMeta          Token mintMeta account
 * @param programId         SPL Token program account
 *
 * @return Instruction to add to a transaction
 */
export function createMintMetaAccountInstruction(
    payer: PublicKey,
    mint: PublicKey,
    mintMeta: PublicKey,
    programId = TOKEN_PROGRAM_ID
): TransactionInstruction {
    const keys = [
        {pubkey: payer, isSigner: true, isWritable: true},
        {pubkey: mint, isSigner: false, isWritable: true},
        {pubkey: mintMeta, isSigner: false, isWritable: true},
        {pubkey: programId, isSigner: false, isWritable: false},
        {pubkey: SYSTEM_PROGRAM_ID, isSigner: false, isWritable: false},
        {pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false},
    ];

    const data = Buffer.alloc(createMintMetaAccountInstructionData.span);

    createMintMetaAccountInstructionData.encode(
        {
            instruction: TokenInstruction.CreateMintMetaAccount,
        },
        data
    );

    return new TransactionInstruction({ keys, programId, data });
}

/** A decoded, valid CreateMintMetaAccount instruction */
export interface DecodedCreateMintMetaAccountInstruction {
    programId: PublicKey;
    keys: {
        payer: AccountMeta;
        mint: AccountMeta;
        mintMeta: AccountMeta;
        program: AccountMeta;
        system: AccountMeta;
        rent: AccountMeta;
    };
    data: {
        instruction: TokenInstruction.CreateMintMetaAccount;
    };
}

/**
 * Decode an CreateMintMetaAccount instruction and validate it
 *
 * @param instruction Transaction instruction to decode
 * @param programId   SPL Token program account
 *
 * @return Decoded, valid instruction
 */
export function decodeCreateMintMetaAccountInstruction(
    instruction: TransactionInstruction,
    programId = TOKEN_PROGRAM_ID
): DecodedCreateMintMetaAccountInstruction {
    if (!instruction.programId.equals(programId)) throw new TokenInvalidInstructionProgramError();
    if (instruction.data.length !== createMintMetaAccountInstructionData.span) throw new TokenInvalidInstructionDataError();

    const {
        keys: { payer, mint, mintMeta, program, system, rent },
        data,
    } = decodeCreateMintMetaAccountInstructionUnchecked(instruction);
    if (data.instruction !== TokenInstruction.UpdateIcon) throw new TokenInvalidInstructionTypeError();
    if (!payer || !mint || !mintMeta || !program || !system || !rent) throw new TokenInvalidInstructionKeysError();

    // TODO: key checks?

    return {
        programId,
        keys: {
            payer,
            mint,
            mintMeta,
            program,
            system,
            rent
        },
        data,
    };
}

/** A decoded, non-validated CreateMintMetaAccount instruction */
export interface DecodedCreateMintMetaAccountInstructionUnchecked {
    programId: PublicKey;
    keys: {
        payer: AccountMeta | undefined;
        mint: AccountMeta | undefined;
        mintMeta: AccountMeta | undefined;
        program: AccountMeta | undefined;
        system: AccountMeta | undefined;
        rent: AccountMeta | undefined;
    };
    data: {
        instruction: number;
    };
}

/**
 * Decode an CreateMintMetaAccount instruction without validating it
 *
 * @param instruction Transaction instruction to decode
 *
 * @return Decoded, non-validated instruction
 */
export function decodeCreateMintMetaAccountInstructionUnchecked({
    programId,
    keys: [payer, mint, mintMeta, program, system, rent],
    data,
}: TransactionInstruction): DecodedCreateMintMetaAccountInstructionUnchecked {
    const { instruction } =
        createMintMetaAccountInstructionData.decode(data);

    return {
        programId,
        keys: {
            payer,
            mint,
            mintMeta,
            program,
            system,
            rent
        },
        data: {
            instruction,
        },
    };
}
