import { PublicKey } from '@com.put/web3.js';

/** Address of the PPL Token program */
export const TOKEN_PROGRAM_ID = new PublicKey('PutToken11111111111111111111111111111111111');

/** Address of the PPL Associated Token Account program */
export const ASSOCIATED_TOKEN_PROGRAM_ID = new PublicKey('PutATA1111111111111111111111111111111111111');

/** Address of the special mint for wrapped native PUT in ppl-token */
export const NATIVE_MINT = new PublicKey('Put1111111111111111111111111111111111111111');

/** Address of the special mint for wrapped native PUT in ppl-token */
export const NATIVE_MINT_METS = new PublicKey('PutMeta111111111111111111111111111111111111');


export const SYSTEM_PROGRAM_ID = new PublicKey('11111111111111111111111111111111');


// /** Check that the token program provided is not `Tokenkeg...`, useful when using extensions */
// export function programSupportsExtensions(programId: PublicKey): boolean {
//     if (programId === TOKEN_PROGRAM_ID) {
//         return false;
//     } else {
//         return true;
//     }
// }
