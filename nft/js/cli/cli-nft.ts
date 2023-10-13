import {CLUSTERS, DEFAULT_CLUSTER, RENT_PROGRAM_ID, SYSTEM_PROGRAM_ID} from "./constants";
import {Keypair} from "@put/web3.js";
import {mintTo} from "../actions/mintTo";
import {setAuthority} from "../actions/setAuthority";
import {update} from "../actions/update";
import {publicKey} from "@metaplex-foundation/beet-put";
import {BN} from "bn.js";
import * as buffer from "buffer";
import {covertDataToMintInfo, covertDataToNftInfo} from "../unpackHelper/helper";

const fs = require("fs");

const { program } = require('commander');
const log = require('loglevel');

const {
    clusterApiUrl,
    Connection,
    PublicKey,
    sendAndConfirmTransaction,
    Transaction,
    TransactionInstruction,
    SystemProgram,
    bs58
} = require('@put/web3.js');
const {createInitializeMintInstruction, InitializeMintArgs, initializeMintArgsBeet} = require("../src/generated");
// import {sendAndConfirmTransaction} from "../../../../token-swap/js/src/util/send-and-confirm-transaction";

program.version('1.0.0');
log.setLevel('info');


programCommand('nft-info')
    .requiredOption('-a, --address <string>', 'the address of nft')
    // eslint-disable-next-line @typescript-eslint/no-unused-vars
    .action(async (directory, cmd) => {
        const {
            keypair,
            env,
            address,
        } = cmd.opts();
        const connection = new Connection(getCluster("devnet"));
        // const sender = loadWalletKey(keypair);

        const info = await connection.getAccountInfo(new PublicKey(address), "finalized");

        const nftInfo = covertDataToNftInfo(info.data)

        log.info("mint: ", nftInfo.mint)
        log.info("owner: ", nftInfo.owner)
        log.info("state: ", nftInfo.state)
        log.info("hasCloseAuthority: ", nftInfo.hasCloseAuthority)
        log.info("closeAuthority: ", nftInfo.closeAuthority)
        log.info("tokenId: ",  nftInfo.tokenId)
        log.info("tokenUri: ", nftInfo.tokenUri)
    });

programCommand('mint-info')
    .requiredOption('-a, --address <string>', 'the address of mint')
    // eslint-disable-next-line @typescript-eslint/no-unused-vars
    .action(async (directory, cmd) => {
        const {
            keypair,
            env,
            address,
        } = cmd.opts();
        const connection = new Connection(getCluster("devnet"));
        // const sender = loadWalletKey(keypair);

        const info = await connection.getAccountInfo(new PublicKey(address), "finalized");

        const mintInfo = covertDataToMintInfo(info.data)

        log.info("mintAuthority: ", mintInfo.mintAuthority)
        log.info("supply: ", mintInfo.supply)
        log.info("totalSupply: ", mintInfo.totalSupply)
        log.info("isInitialized: ", mintInfo.isInitialized)
        log.info("name: ", mintInfo.name)
        log.info("symbol: ",  mintInfo.symbol)
        log.info("hasFreezeAuthority: ", mintInfo.hasFreezeAuthority)
        log.info("freezeAuthority: ", mintInfo.freezeAuthority)
        log.info("ICON uri: ", mintInfo.iconUri)
});

programCommand('update')
    .requiredOption('-t, --type <string>', 'the set authority type')
    .requiredOption('-a, --address <string>', 'the address data will be update')
    .requiredOption('-v, --value <string>', 'the new value to update')
    // eslint-disable-next-line @typescript-eslint/no-unused-vars
    .action(async (directory, cmd) => {
        const {
            keypair,
            env,
            type,
            address,
            value,
        } = cmd.opts();
        if (type !== "icon" && type !==  "asset") {
            log.error("invalid type", type)
            return
        }
        const connection = new Connection(getCluster("devnet"));
        const sender = loadWalletKey(keypair);

        await update(connection, sender, new PublicKey(address),type, value)
});

programCommand('set-authority')
    .requiredOption('-t, --type <string>', 'the set authority type')
    .requiredOption('-a, --address <string>', 'the address to set authority')
    .option('-na, --new-authority <string>', 'new authority')
    // eslint-disable-next-line @typescript-eslint/no-unused-vars
    .action(async (directory, cmd) => {
        const {
            keypair,
            env,
            type,
            address,
            newAuthority,
        } = cmd.opts();
        if (type !== "freeze" && type !==  "close" && type !== "mint") {
            log.error("wrong type ?", type)
            return
        }
        const connection = new Connection(getCluster("devnet"));
        const sender = loadWalletKey(keypair);

        let newAuthorityPubkey = null;
        if(newAuthority !== undefined) {
            newAuthorityPubkey = new PublicKey(newAuthority)
        }
        await setAuthority(connection, sender, type, new PublicKey(address), newAuthorityPubkey)
});

programCommand('create-mint')
  .requiredOption('-ts, --total-supply <number>', 'the nft total supply')
  .requiredOption('-n, --name <string>', 'the nft name')
  .requiredOption('-s, --symbol <string>', 'the nft symbol')
  .option('-iu, --icon-uri <string>', 'the nft icon-uri')
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  .action(async (directory, cmd) => {
    const {
      keypair,
      env,
      totalSupply,
      name,
      symbol,
      iconUri,
    } = cmd.opts();
    const connection = new Connection(getCluster("devnet"));
    // const instructions: TransactionInstruction[] = [];
    const newMint = Keypair.generate();

    log.info("totalSupply:", totalSupply);
      log.info("newMint:", newMint.publicKey.toBase58());
    const sender = loadWalletKey(keypair)
    log.info("sender")


    // const transaction = new Transaction().add(SystemProgram.createAccount({
    //   fromPubkey: sender.publicKey,
    //   newAccountPubkey: newMint.publicKey,
    //   lamports: 0,
    //   space: 0,
    //   programId: SYSTEM_PROGRAM_ID
    // }));
    // const signature1 = await sendAndConfirmTransaction(connection, createAccountTx, [sender], null);
    //   log.info("signature1", signature1)
      log.info("come to here")
   const transaction = new Transaction().add(createInitializeMintInstruction(
        {mint: newMint.publicKey, mintAuthority: sender.publicKey, systemProgram: SYSTEM_PROGRAM_ID, rent: RENT_PROGRAM_ID},
        {initializeMintArgs:
                {
                    totalSupply: totalSupply,
                    mintAuthority: sender.publicKey,
                    freezeAuthority: null,
                    name: name,
                    symbol: symbol,
                    iconUri: iconUri,
                }
        }
    ));
    const signature = await sendAndConfirmTransaction(connection, transaction, [sender, newMint], null);
    log.info("signature", signature)
});

programCommand('mint-to')
    .requiredOption('-t, --token <string>', 'the mint address')
    .requiredOption('-u, --token-uri <string>', 'the nft token uri')
    // eslint-disable-next-line @typescript-eslint/no-unused-vars
    .action(async (directory, cmd) => {
        const {
            keypair,
            env,
            token,
            tokenUri,
        } = cmd.opts();

        const connection = new Connection(getCluster("devnet"));
        // const instructions: TransactionInstruction[] = [];
        const newMint = Keypair.generate();

        const mint = new PublicKey(token);

        const sender = loadWalletKey(keypair);

        const nftKey = mintTo(connection, sender, mint, tokenUri);
        log.info("mint new NFT:", nftKey)
});

function programCommand(name: string) {
  return program
      .command(name)
      .option(
          '-e, --env <string>',
          'put cluster env name',
          'devnet', //mainnet-beta, testnet, devnet
      )
      .option(
          '-k, --keypair <path>',
          `put wallet location`,
          '--keypair not provided',
      )
      .option('-l, --log-level <string>', 'log level', setLogLevel);
}

// eslint-disable-next-line @typescript-eslint/no-unused-vars
function setLogLevel(value, prev) {
  if (value === undefined || value === null) {
    return;
  }
  log.info('setting the log value to: ' + value);
  log.setLevel(value);
}

function loadWalletKey(keypair): Keypair {
    if (!keypair || keypair == '') {
        throw new Error('Keypair is required!');
    }

    const decodedKey = new Uint8Array(
        keypair.endsWith('.json') && !Array.isArray(keypair)
            ? JSON.parse(fs.readFileSync(keypair).toString())
            : bs58.decode(keypair),
    );

    const loaded = Keypair.fromSecretKey(decodedKey);
    log.info(`wallet public key: ${loaded.publicKey}`);
    return loaded;
}

function getCluster(name: string): string {
    if (name === '') {
        log.info('Using cluster', DEFAULT_CLUSTER.name);
        return DEFAULT_CLUSTER.url;
    }

    for (const cluster of CLUSTERS) {
        if (cluster.name === name) {
            log.info('Using cluster', cluster.name);
            return cluster.url;
        }
    }

    throw new Error(`Could not get cluster: ${name}`);
    return null;
}

program.parse(process.argv);