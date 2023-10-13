import chai, { expect } from 'chai';
import chaiAsPromised from 'chai-as-promised';
import type { Connection, PublicKey, Signer } from '@com.put/web3.js';
import { Keypair } from '@com.put/web3.js';
import { createMint } from '../../src';
import { TEST_PROGRAM_ID, newAccountWithLamports, getConnection } from '../common';

chai.use(chaiAsPromised);

const TEST_TOKEN_DECIMALS = 2;
describe('Amount', () => {
    let connection: Connection;
    let payer: Signer;
    let mint: PublicKey;
    let mintAuthority: Keypair;
    before(async () => {
        connection = await getConnection();
        payer = await newAccountWithLamports(connection, 1000000000n);
        mintAuthority = Keypair.generate();
        const mintKeypair = Keypair.generate();
        mint = await createMint(
            connection,
            payer,
            "",
            "",
            "",
            mintAuthority.publicKey,
            mintAuthority.publicKey,
            TEST_TOKEN_DECIMALS,
            mintKeypair,
            undefined,
            TEST_PROGRAM_ID
        );
    });
});
