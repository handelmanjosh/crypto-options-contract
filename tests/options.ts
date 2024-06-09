import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Options } from "../target/types/options";
import { Keypair, PublicKey } from "@solana/web3.js";
import {createMint, getAssociatedTokenAddressSync, getOrCreateAssociatedTokenAccount, mintTo} from "@solana/spl-token";

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
    await program.methods.initialize().rpc();
  });
  it("creates option mint", async () => {
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
    await program.methods.create(new anchor.BN(date), 200, 400 * 10 ** OPTION_DECIMALS).accounts({
      signer: wallet.publicKey,
      underlyingMint,
      userUnderlyingTokenAccount,
      underlyingTokenAccount,
      optionMint: optionMint.publicKey,
      userOptionTokenAccount,
      optionDataAccount,
      programAuthority,
    }).rpc();
    const optionData = await program.account.optionDataAccount.fetch(optionDataAccount);
  });
});
