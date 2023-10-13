import {
    ConfirmOptions,
    Connection,
    PublicKey,
    sendAndConfirmTransaction,
    Signer,
    Transaction,
} from '@put/web3.js';
import log from "loglevel";
import {
    createUpdateInstruction,
} from '../src/generated'

/**
 * Update mint or nft account data
 *
 * @param connection        Connection to use
 * @param ownerAccount      the owner account of the account that authority will be set
 * @param address           the address data will be update
 * @param updateType        update type,value takes 'icon'、'asset'
 * @param value             the value will set in address data
 * @param confirmOptions    Options for confirming the transaction
 *
 */
export async function update(
    connection: Connection,
    ownerAccount: Signer,
    address: PublicKey,
    updateType: string,
    value: string,
    confirmOptions?: ConfirmOptions,
) {
    let updateTypeVal = null;
    if (updateType === "icon") {
        updateTypeVal = {__kind : "Icon", iconUri: value}
    } else if (updateType === "asset") {
        updateTypeVal = {__kind : "", NftAsset: value}
    } else {
        log.error("invalid update type")
        return
    }

    const transaction = new Transaction().add(createUpdateInstruction(
        {addressPubkey: address, owner: ownerAccount.publicKey},
        {updateType: updateTypeVal}
    ));
    log.info("come to here")
    const signature = await sendAndConfirmTransaction(connection, transaction, [ownerAccount], confirmOptions);
    log.info("signature", signature)
}
