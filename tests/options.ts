import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Options } from "../target/types/options";
import { Keypair, PublicKey, LAMPORTS_PER_SOL } from "@solana/web3.js";
import {createMint, getAssociatedTokenAddressSync, getOrCreateAssociatedTokenAccount, mintTo} from "@solana/spl-token";
import { assert } from "chai";

describe("options", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const wallet = provider.wallet as anchor.Wallet
  const program = anchor.workspace.Options as Program<Options>;
  const [programAuthority] = PublicKey.findProgramAddressSync(
    [Buffer.from("auth")],
    program.programId,
  )
  const OPTION_DECIMALS: number = 6;
  const MINT_AMOUNT: number = 100000 * 10 ** OPTION_DECIMALS;
  const mintToken = async () => {
    const mint = await createMint(
      provider.connection,
      wallet.payer,
      wallet.publicKey,
      null,
      OPTION_DECIMALS,
    )
    const tokenAccount = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      wallet.payer,
      mint,
      wallet.publicKey,
    );
    await mintTo(
      provider.connection,
      wallet.payer,
      mint,
      tokenAccount.address,
      wallet.payer,
      MINT_AMOUNT
    );
    return { tokenAccount: tokenAccount.address, mint }
  };
  it("initialized", async () => {
    // Add your test here.
    await program.methods.initialize().accounts({
      signer: wallet.publicKey,
      programAuthority
    }).rpc();
  });
  const createOption = async () => {
    const { mint: underlyingMint, tokenAccount: userUnderlyingTokenAccount } = await mintToken();
    const [underlyingTokenAccount] = PublicKey.findProgramAddressSync(
      [Buffer.from("underlying_token"), underlyingMint.toBuffer()],
      program.programId
    );
    const optionMint = Keypair.generate();
    const userOptionTokenAccount = getAssociatedTokenAddressSync(optionMint.publicKey, wallet.publicKey);
    const [optionDataAccount] = PublicKey.findProgramAddressSync(
      [Buffer.from("option_data_account"), optionMint.publicKey.toBuffer()],
      program.programId,
    );
    const date = Date.now() + 1000000;
    const accounts = {
      signer: wallet.publicKey,
      underlyingMint,
      userUnderlyingTokenAccount,
      underlyingTokenAccount,
      optionMint: optionMint.publicKey,
      userOptionTokenAccount,
      optionDataAccount,
      programAuthority,
    }
    // for (const account in accounts) {
    //   console.log(`${account}: ${accounts[account].toString()}`);
    // }
    await program.methods.create(new anchor.BN(date), new anchor.BN(200), new anchor.BN(400 * 10 ** OPTION_DECIMALS)).accounts(accounts).signers([optionMint]).rpc();
    return {...accounts, date};
  }
  it("creates option mint", async () => {
    const { optionDataAccount, date, underlyingMint } = await createOption();
    const optionData = await program.account.optionDataAccount.fetch(optionDataAccount);
    console.log(optionData);
    console.log(optionData.endTime.toNumber() === date);
    assert(optionData.amountUnexercised.toNumber() === 400 * 10 ** OPTION_DECIMALS);
    assert(optionData.strikePrice.toNumber() === 200);
    assert(optionData.underlyingMint.equals(underlyingMint));
    assert(optionData.creator.equals(wallet.publicKey));
  });
  it("lists multiple of same", async () => {
    const {
      optionDataAccount, underlyingMint, userUnderlyingTokenAccount, 
      underlyingTokenAccount, optionMint, userOptionTokenAccount, } = await createOption();
    const [programHolderAccount] = PublicKey.findProgramAddressSync(
      [Buffer.from("holder_account"), optionMint.toBuffer()],
      program.programId,
    );
    const [listAccount] = PublicKey.findProgramAddressSync(
      [Buffer.from("listing"), optionMint.toBuffer(), wallet.publicKey.toBuffer(), Buffer.from(LAMPORTS_PER_SOL / 3)],
      program.programId,
    )
    await program.methods.list(new anchor.BN(400), new anchor.BN(LAMPORTS_PER_SOL / 4)).accounts({
      signer: wallet.publicKey,
      optionMint,
      userOptionTokenAccount,
      optionDataAccount,
      programHolderAccount,
      listAccount,
      programAuthority,
    }).signers([]).rpc();
  });
});
