import {
  Connection,
  Keypair,
  Signer,
  PublicKey,
  Transaction,
  TransactionInstruction,
  TransactionSignature,
  ConfirmOptions,
  sendAndConfirmRawTransaction,
  sendAndConfirmTransaction,
  RpcResponseAndContext,
  SimulatedTransactionResponse,
  Commitment,
  LAMPORTS_PER_SOL,
  SystemProgram,
  SYSVAR_RENT_PUBKEY,
  clusterApiUrl
} from "@solana/web3.js"
import * as bs58 from 'bs58'
import fs from 'fs'
import * as anchor from '@project-serum/anchor'
import {AccountLayout,MintLayout,TOKEN_PROGRAM_ID,Token,ASSOCIATED_TOKEN_PROGRAM_ID} from "@solana/spl-token";
import { program } from 'commander';
import log from 'loglevel';

program.version('0.0.1');
log.setLevel('info');

// const programId = new PublicKey('AirdfxxqajyegRGW1RpY5JfPyYiZ2Z9WYAZxmhKzxoKo')
const programId = new PublicKey('AiAK8Z8eBPmtH9uGVawmZCvyYhnkmngsuvQEJAPV8Rdf')
const idl=JSON.parse(fs.readFileSync('src/solana_anchor.json','utf8'))

const confirmOption : ConfirmOptions = {
    commitment : 'finalized',
    preflightCommitment : 'finalized',
    skipPreflight : false
}

const sleep = (ms : number) => {
    return new Promise(resolve => setTimeout(resolve, ms));
};

function loadWalletKey(keypair : any): Keypair {
  if (!keypair || keypair == '') {
    throw new Error('Keypair is required!');
  }
  const loaded = Keypair.fromSecretKey(
    new Uint8Array(JSON.parse(fs.readFileSync(keypair).toString())),
  );
  log.info(`wallet public key: ${loaded.publicKey}`);
  return loaded;
}

const getTokenWallet = async (
  wallet: anchor.web3.PublicKey,
  mint: anchor.web3.PublicKey
    ) => {
  return (
    await anchor.web3.PublicKey.findProgramAddress(
      [wallet.toBuffer(), TOKEN_PROGRAM_ID.toBuffer(), mint.toBuffer()],
      ASSOCIATED_TOKEN_PROGRAM_ID
    )
  )[0];
}

const createAssociatedTokenAccountInstruction = (
  associatedTokenAddress: PublicKey,
  payer: PublicKey,
  walletAddress: PublicKey,
  splTokenMintAddress: PublicKey
    ) => {
  const keys = [
    { pubkey: payer, isSigner: true, isWritable: true },
    { pubkey: associatedTokenAddress, isSigner: false, isWritable: true },
    { pubkey: walletAddress, isSigner: false, isWritable: false },
    { pubkey: splTokenMintAddress, isSigner: false, isWritable: false },
    {
      pubkey: SystemProgram.programId,
      isSigner: false,
      isWritable: false,
    },
    { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    {
      pubkey: SYSVAR_RENT_PUBKEY,
      isSigner: false,
      isWritable: false,
    },
  ];
  return new TransactionInstruction({
    keys,
    programId: ASSOCIATED_TOKEN_PROGRAM_ID,
    data: Buffer.from([]),
  });
}

async function getDecimalsOfToken(conn : Connection, mint : PublicKey){
  let resp = await conn.getAccountInfo(mint)
  // console.log(resp)
  let accountData = MintLayout.decode(Buffer.from(resp!.data))
  return accountData.decimals
}

programCommand('init_pool')
  .requiredOption(
    '-k, --keypair <path>',
    'Solana wallet location'
  )
  .requiredOption(
    '-i, --info <path>',
    'Schedule info location'
  )
  .action(async (directory,cmd)=>{
    try{
    const {env,keypair,info} = cmd.opts()
    const conn = new Connection(clusterApiUrl(env))
    const owner = loadWalletKey(keypair)
    const wallet = new anchor.Wallet(owner)
    const provider = new anchor.Provider(conn,wallet,confirmOption)
    const program = new anchor.Program(idl,programId,provider)
    const rand = Keypair.generate().publicKey;
    const [pool, bump] = await PublicKey.findProgramAddress([rand.toBuffer()],programId)
    let transaction = new Transaction()
    const infoJson = JSON.parse(fs.readFileSync(info).toString())
    const tokenMint = new PublicKey(infoJson.token)
    // console.log("token mint", tokenMint)
    const tokenAccount = await getTokenWallet(pool, tokenMint)
    transaction.add(createAssociatedTokenAccountInstruction(tokenAccount, owner.publicKey, pool, tokenMint))
    const decimals = Math.pow(10,await getDecimalsOfToken(conn,tokenMint))
    let newSchedule : any[] = [];
    (infoJson.schedule as any[]).map(item => {
      let amount = Number(item.amount) * decimals
      let time = (new Date(item.time)).getTime()/1000
      newSchedule.push({
        airdropTime : new anchor.BN(time),
        airdropAmount : new anchor.BN(amount),
      })
    })
    transaction.add(program.instruction.initPool(
      new anchor.BN(bump),
      newSchedule,
      new anchor.BN(infoJson.period),
      infoJson.symbol,
      {
        accounts:{
          owner : owner.publicKey,
          pool : pool,
          rand : rand,
          rewardMint : tokenMint,
          rewardAccount : tokenAccount,
          systemProgram : SystemProgram.programId
        }
      }
    ))
    const hash = await sendAndConfirmTransaction(conn, transaction, [owner], confirmOption)
    console.log("POOL : "+pool.toBase58())
    console.log("Transaction ID : " + hash)
    }catch(err){
      console.log(err)
    }
  })

programCommand('get_pool')
  .option(
    '-p, --pool <string>',
    'pool address'
  )
  .action(async (directory,cmd)=>{
    const {env, pool} = cmd.opts()
    const conn = new Connection(clusterApiUrl(env))
    const poolAddress = new PublicKey(pool)
    const wallet = new anchor.Wallet(Keypair.generate())
    const provider = new anchor.Provider(conn,wallet,confirmOption)
    const program = new anchor.Program(idl,programId,provider)
    const poolData = await program.account.pool.fetch(poolAddress)
    const resp = await conn.getTokenAccountBalance(poolData.rewardAccount, "max")
    const amount = resp.value.uiAmountString
    const decimals = Math.pow(10,resp.value.decimals)
    console.log("        Pool Data")
    console.log("Owner : " + poolData.owner.toBase58())
    console.log("Token : " + poolData.rewardMint.toBase58())
    console.log("Token Address : " + poolData.rewardAccount.toBase58())
    console.log("period : " + poolData.period.toNumber() + "s")
    console.log("Collection symbol : " + poolData.stakeCollection)
    console.log("when                   amount");
    (poolData.schedule as any[]).map((item) => {
      console.log((new Date(item!.airdropTime*1000)).toLocaleString(),"      ",item!.airdropAmount/decimals)
    })
    console.log("")
  })

function programCommand(name: string) {
  return program
    .command(name)
    .option(
      '-e, --env <string>',
      'Solana cluster env name',
      'devnet',
    )
    .option('-l, --log-level <string>', 'log level', setLogLevel);
}

function setLogLevel(value : any, prev : any) {
  if (value === undefined || value === null) {
    return;
  }
  console.log('setting the log value to: ' + value);
  log.setLevel(value);
}

program.parse(process.argv)