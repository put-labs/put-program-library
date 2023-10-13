import type { Connection, PublicKey, Signer } from '@com.put/web3.js';
import { Keypair } from '@com.put/web3.js';

import chai, { expect } from 'chai';
import chaiAsPromised from 'chai-as-promised';
chai.use(chaiAsPromised);

import {
    createMint,
    getMint,
    getMintMeta,
    updateTokenSymbol,
    updateTokenName,
    updateTokenIcon,
    NATIVE_MINT,
} from '../../src';

import { TEST_PROGRAM_ID, newAccountWithLamports, getConnection } from '../common';

const TEST_TOKEN_DECIMALS = 2;
describe('updateMintMeta', () => {
    let connection: Connection;
    let payer: Signer;
    let mint: PublicKey;
    let mintAuthority: Keypair;
    let mintKeypair: Keypair;

    before(async () => {
        connection = await getConnection();
        payer = await newAccountWithLamports(connection, 1000000000n);
        mintAuthority = Keypair.generate();
        mintKeypair = Keypair.generate();
        mint = await createMint(
            connection,
            payer,
            "BTC",
            "BTCoin",
            "https://s2.coinmarketcap.com/static/img/coins/64x64/1.png",
            mintAuthority.publicKey,
            mintAuthority.publicKey,
            TEST_TOKEN_DECIMALS,
            mintKeypair,
            undefined,
            TEST_PROGRAM_ID
        );

        const mintInfo = await getMint(connection, mint, undefined, TEST_PROGRAM_ID);

        expect(mintInfo.mintAuthority).to.eql(mintAuthority.publicKey);
        expect(mintInfo.supply).to.eql(BigInt(0));
        expect(mintInfo.decimals).to.eql(TEST_TOKEN_DECIMALS);
        expect(mintInfo.isInitialized).to.be.true;
        expect(mintInfo.freezeAuthority).to.eql(mintAuthority.publicKey);

        const mintMeta = await getMintMeta(connection, mint, undefined, TEST_PROGRAM_ID);

        expect(mintMeta.authorityOption).to.eql(mintAuthority.publicKey);
        expect(mintMeta.symbol).to.eql("BTC");
        expect(mintMeta.name).to.eql("BTCoin");
        expect(mintMeta.icon).to.eql("https://s2.coinmarketcap.com/static/img/coins/64x64/1.png");
    });

    it('updateSymbol', async () => {
        let TransactionSignature = await updateTokenSymbol(
            connection,
            payer,
            mintKeypair.publicKey,
            mintAuthority,
            "uBTC",
            undefined,
            undefined,
            TEST_PROGRAM_ID
        );
        console.log("TransactionSignature:",TransactionSignature);

        const mintMeta = await getMintMeta(connection, mint, undefined, TEST_PROGRAM_ID);

        expect(mintMeta.authorityOption).to.eql(mintAuthority.publicKey);
        expect(mintMeta.symbol).to.eql("uBTC");

    });

    it('updateName', async () => {
        let TransactionSignature = await updateTokenName(
            connection,
            payer,
            mintKeypair.publicKey,
            mintAuthority,
            "uBTCoin",
            undefined,
            undefined,
            TEST_PROGRAM_ID
        );
        console.log("TransactionSignature:",TransactionSignature);

        const mintMeta = await getMintMeta(connection, mint, undefined, TEST_PROGRAM_ID);

        expect(mintMeta.authorityOption).to.eql(mintAuthority.publicKey);
        expect(mintMeta.name).to.eql("uBTCoin");

    });

    it('updateIcon', async () => {
        let TransactionSignature = await updateTokenIcon(
            connection,
            payer,
            mintKeypair.publicKey,
            mintAuthority,
            "uhttps://s2.coinmarketcap.com/static/img/coins/64x64/1.png",
            undefined,
            undefined,
            TEST_PROGRAM_ID
        );
        console.log("TransactionSignature:",TransactionSignature);

        const mintMeta = await getMintMeta(connection, mint, undefined, TEST_PROGRAM_ID);

        expect(mintMeta.authorityOption).to.eql(mintAuthority.publicKey);
        expect(mintMeta.icon).to.eql("uhttps://s2.coinmarketcap.com/static/img/coins/64x64/1.png");

    });

    it('getNativeMintMeta', async () => {

        const mintMeta = await getMintMeta(connection, NATIVE_MINT, undefined, TEST_PROGRAM_ID);
        console.log("mintMeta.symbol:",mintMeta.symbol);
        expect(mintMeta.authorityOption).to.be.null;
        expect(mintMeta.symbol).to.eql("WPUT");
        expect(mintMeta.name).to.eql("Wrap PUT");
        // expect(mintMeta.icon).to.eql("http://scan.puttest.com:9898/static/media/dark-explorer-logo.3d27a63f.svg");

    });

});
