import * as buffer from "buffer";
import {BN} from "bn.js";
import { PublicKey } from "@put/web3.js";

export function covertDataToMintInfo(data) : any {
    const mintAuthority = data.slice(0, 32);
    const supply = buffer.Buffer.from(data.slice(32, 32 + 8));
    const totalSupply = buffer.Buffer.from(data.slice(40, 40 + 8));
    const isInitialized = data.slice(48, 48 + 1)[0] == 1;
    const name = data.slice(49, 49 + 32).toString();
    const symbol = data.slice(81, 81 + 8).toString();
    const hasFrozen = data.slice(89, 89 + 1)[0] == 1;
    const freezeAuthority = data.slice(90, 90 + 32);
    const uri = data.slice(122, 200 + 122).toString();

    let bnSupply = new BN(supply, "le")
    let bnTotalSupply = new BN(totalSupply, "le")

    return {
        "mintAuthority": new PublicKey(mintAuthority).toBase58(),
        "supply": bnSupply.toNumber(),
        "totalSupply": bnTotalSupply.toNumber(),
        "isInitialized": isInitialized,
        "name": name,
        "symbol": symbol,
        "hasFreezeAuthority": hasFrozen,
        "freezeAuthority": hasFrozen ? new PublicKey(freezeAuthority).toBase58() : "None",
        "iconUri": uri
    }
}

export function covertDataToNftInfo(data) : any {
    const mint = data.slice(0, 32);
    const owner = data.slice(32, 32 + 32);
    const state = data.slice(64, 64 + 1);
    const hasCloseAuthority = data.slice(65, 65 + 1) == 1;
    const closeAuthority = data.slice(66, 66 + 32);
    const tokenId = data.slice(98, 98 + 8);
    const uri = data.slice(106, 106 + 200).toString();

    let bnTokenId = new BN(tokenId, "le")

    let nftState;
    if (state[0] == 0) {
        nftState = "Uninitialized"
    } else if (state[0] == 1){
        nftState = "Initialized"
    } else if (state[0] == 2){
        nftState = "Freeze"
    }

    return {
        "mint": new PublicKey(mint).toBase58(),
        "owner":new PublicKey(owner).toBase58(),
        "state": nftState,
        "hasCloseAuthority": hasCloseAuthority,
        "closeAuthority": hasCloseAuthority ? new PublicKey(closeAuthority).toBase58() : "None",
        "tokenId": bnTokenId.toNumber(),
        "tokenUri": uri
    }
}