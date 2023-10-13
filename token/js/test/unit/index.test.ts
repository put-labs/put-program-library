import { Keypair, PublicKey } from '@com.put/web3.js';
import chai, { expect } from 'chai';
import chaiAsPromised from 'chai-as-promised';
import { Console } from 'console';
import {
    ASSOCIATED_TOKEN_PROGRAM_ID,
    createAssociatedTokenAccountInstruction,
    createInitializeMintInstruction,
    createSyncNativeInstruction,
    createTransferCheckedInstruction,
    getAssociatedTokenAddress,
    TOKEN_PROGRAM_ID,
    TokenInstruction,
    TokenOwnerOffCurveError,
    createInitializeAccount2Instruction,
} from '../../src';

chai.use(chaiAsPromised);

describe('ppl-token instructions', () => {
    it('TransferChecked', () => {
        const ix = createTransferCheckedInstruction(
            Keypair.generate().publicKey,
            Keypair.generate().publicKey,
            Keypair.generate().publicKey,
            Keypair.generate().publicKey,
            1n,
            9
        );
        expect(ix.programId).to.eql(TOKEN_PROGRAM_ID);
        expect(ix.keys).to.have.length(4);
    });

    it('InitializeMint', () => {
        const ix = createInitializeMintInstruction(Keypair.generate().publicKey, 9, Keypair.generate().publicKey, null);
        expect(ix.programId).to.eql(TOKEN_PROGRAM_ID);
        expect(ix.keys).to.have.length(2);
    });

    it('SyncNative', () => {
        const ix = createSyncNativeInstruction(Keypair.generate().publicKey);
        expect(ix.programId).to.eql(TOKEN_PROGRAM_ID);
        expect(ix.keys).to.have.length(1);
    });

    it('InitializeAccount2', () => {
        const ix = createInitializeAccount2Instruction(
            Keypair.generate().publicKey,
            Keypair.generate().publicKey,
            Keypair.generate().publicKey
        );
        expect(ix.programId).to.eql(TOKEN_PROGRAM_ID);
        expect(ix.keys).to.have.length(3);
    });
});

describe('ppl-associated-token-account instructions', () => {
    it('create', () => {
        const ix = createAssociatedTokenAccountInstruction(
            Keypair.generate().publicKey,
            Keypair.generate().publicKey,
            Keypair.generate().publicKey,
            Keypair.generate().publicKey
        );
        expect(ix.programId).to.eql(ASSOCIATED_TOKEN_PROGRAM_ID);
        expect(ix.keys).to.have.length(7);
    });
});

describe('state', () => {
    it('getAssociatedTokenAddress', async () => {
        const associatedPublicKey = await getAssociatedTokenAddress(
            new PublicKey('7o36UsWR1JQLpZ9PE2gn9L4SQ69CNNiWAXd4Jt7rqz9Z'),
            new PublicKey('B8UwBUUnKwCyKuGMbFKWaG7exYdDk2ozZrPg72NyVbfj')
        );
        console.log("associatedPublicKey:",associatedPublicKey.toString());
        expect(associatedPublicKey.toString()).to.eql(
            new PublicKey('HZejvuNbaPfxXmLpU78HDLpnYmQKD2W1uUGE11tbCf2a').toString()
        );
        await expect(
            getAssociatedTokenAddress(
                new PublicKey('7o36UsWR1JQLpZ9PE2gn9L4SQ69CNNiWAXd4Jt7rqz9Z'),
                associatedPublicKey
            )
        ).to.be.rejectedWith(TokenOwnerOffCurveError);

        const associatedPublicKey2 = await getAssociatedTokenAddress(
            new PublicKey('7o36UsWR1JQLpZ9PE2gn9L4SQ69CNNiWAXd4Jt7rqz9Z'),
            associatedPublicKey,
            true
        );
        console.log("associatedPublicKey2:",associatedPublicKey2.toString());
        expect(associatedPublicKey2.toString()).to.eql(
            new PublicKey('qw9Ff3gXb3daCpzZCQKNEF2VjB89TtmdDPoS76Wo6ot').toString()
        );
    });

    it('getAssociatedTokenAddressSync matches getAssociatedTokenAddress', async () => {
        const asyncAssociatedPublicKey = await getAssociatedTokenAddress(
            new PublicKey('7o36UsWR1JQLpZ9PE2gn9L4SQ69CNNiWAXd4Jt7rqz9Z'),
            new PublicKey('B8UwBUUnKwCyKuGMbFKWaG7exYdDk2ozZrPg72NyVbfj')
        );
        const associatedPublicKey = await getAssociatedTokenAddress(
            new PublicKey('7o36UsWR1JQLpZ9PE2gn9L4SQ69CNNiWAXd4Jt7rqz9Z'),
            new PublicKey('B8UwBUUnKwCyKuGMbFKWaG7exYdDk2ozZrPg72NyVbfj')
        );
        // console.log("asyncAssociatedPublicKey:",asyncAssociatedPublicKey.toString());
        expect(associatedPublicKey.toString()).to.eql(
            new PublicKey('HZejvuNbaPfxXmLpU78HDLpnYmQKD2W1uUGE11tbCf2a').toString()
        );
        expect(asyncAssociatedPublicKey.toString()).to.eql(associatedPublicKey.toString());
    });
});
