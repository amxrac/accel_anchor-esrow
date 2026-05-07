#[cfg(test)]
mod tests {

    use {
        crate::tests::tests,
        anchor_lang::{
            prelude::msg, solana_program::program_pack::Pack, AccountDeserialize, InstructionData,
            ToAccountMetas,
        },
        anchor_spl::{
            associated_token::{self, spl_associated_token_account},
            token::spl_token,
        },
        litesvm::LiteSVM,
        litesvm_token::{
            spl_token::ID as TOKEN_PROGRAM_ID, CreateAssociatedTokenAccount, CreateMint, MintTo,
        },
        solana_account::Account,
        solana_address::Address,
        solana_instruction::Instruction,
        solana_keypair::Keypair,
        solana_message::Message,
        solana_native_token::LAMPORTS_PER_SOL,
        solana_pubkey::Pubkey,
        solana_rpc_client::rpc_client::RpcClient,
        solana_sdk_ids::system_program::ID as SYSTEM_PROGRAM_ID,
        solana_signer::Signer,
        solana_transaction::Transaction,
        std::{path::PathBuf, str::FromStr},
    };

    struct TestConfig {
        program: LiteSVM,
        payer: Keypair,
        maker: Pubkey,
        taker: Keypair,
        mint_a: Pubkey,
        mint_b: Pubkey,
        maker_ata_a: Pubkey,
        maker_ata_b: Pubkey,
        taker_ata_a: Pubkey,
        taker_ata_b: Pubkey,
        escrow: Pubkey,
        vault: Pubkey,
        associated_token_program: Pubkey,
        token_program: Pubkey,
        system_program: Pubkey,
    }

    impl TestConfig {
        pub fn make(&mut self, seed: u64, deposit: u64, receive: u64) {
            let ix = Instruction {
                program_id: PROGRAM_ID,
                accounts: crate::accounts::Make {
                    maker: self.maker,
                    mint_a: self.mint_a,
                    mint_b: self.mint_b,
                    maker_ata_a: self.maker_ata_a,
                    escrow: self.escrow,
                    vault: self.vault,
                    associated_token_program: self.associated_token_program,
                    token_program: self.token_program,
                    system_program: self.system_program,
                }
                .to_account_metas(None),
                data: crate::instruction::Make {
                    seed: seed,
                    deposit: deposit,
                    receive: receive,
                }
                .data(),
            };

            let message = Message::new(&[ix], Some(&self.payer.pubkey()));

            let tx = Transaction::new(&[&self.payer], message, self.program.latest_blockhash());

            let result = self.program.send_transaction(tx).unwrap();

            println!("{}", result.pretty_logs());

            // Log transaction details
            println!("\n\nMake transaction sucessfull");
            println!("CUs Consumed: {}", result.compute_units_consumed);
            println!("Tx Signature: {}", result.signature);
        }

        pub fn take(&mut self) {
            let ix = Instruction {
                program_id: PROGRAM_ID,
                accounts: crate::accounts::Take {
                    taker: self.taker.pubkey(),
                    maker: self.maker,
                    mint_a: self.mint_a,
                    mint_b: self.mint_b,
                    taker_ata_a: self.taker_ata_a,
                    taker_ata_b: self.taker_ata_b,
                    maker_ata_b: self.maker_ata_b,
                    escrow: self.escrow,
                    vault: self.vault,
                    associated_token_program: self.associated_token_program,
                    token_program: self.token_program,
                    system_program: self.system_program,
                }
                .to_account_metas(None),
                data: crate::instruction::Take {}.data(),
            };

            let message = Message::new(&[ix], Some(&self.taker.pubkey()));
            let tx = Transaction::new(&[&self.taker], message, self.program.latest_blockhash());

            let result = self.program.send_transaction(tx).unwrap();

            println!("{}", result.pretty_logs());

            // Log transaction details
            println!("\n\nTake transaction sucessfull");
            println!("CUs Consumed: {}", result.compute_units_consumed);
            println!("Tx Signature: {}", result.signature);
        }

        pub fn refund(&mut self) {
            let ix = Instruction {
                program_id: PROGRAM_ID,
                accounts: crate::accounts::Refund {
                    maker: self.maker,
                    mint_a: self.mint_a,
                    maker_ata_a: self.maker_ata_a,
                    escrow: self.escrow,
                    vault: self.vault,
                    token_program: self.token_program,
                    system_program: self.system_program,
                }
                .to_account_metas(None),
                data: crate::instruction::Refund {}.data(),
            };

            let message = Message::new(&[ix], Some(&self.maker));

            let tx = Transaction::new(&[&self.payer], message, self.program.latest_blockhash());

            let result = self.program.send_transaction(tx).unwrap();

            println!("{}", result.pretty_logs());

            // Log transaction details
            println!("\n\nRefund transaction sucessfull");
            println!("CUs Consumed: {}", result.compute_units_consumed);
            println!("Tx Signature: {}", result.signature);
        }

        pub fn assert_vault(&self, amount: u64) {
            // Verify the vault account and escrow account data after the "Make" instruction
            let vault_account = self.program.get_account(&self.vault).unwrap();
            let vault_data = spl_token::state::Account::unpack(&vault_account.data).unwrap();
            assert_eq!(vault_data.amount, amount);
            assert_eq!(vault_data.owner, self.escrow);
            assert_eq!(vault_data.mint, self.mint_a);
        }

        pub fn assert_escrow(&self, amount: u64) {
            let escrow_account = self.program.get_account(&self.escrow).unwrap();
            let escrow_data =
                crate::state::Escrow::try_deserialize(&mut escrow_account.data.as_ref()).unwrap();
            assert_eq!(escrow_data.seed, 123u64);
            assert_eq!(escrow_data.maker, self.maker);
            assert_eq!(escrow_data.mint_a, self.mint_a);
            assert_eq!(escrow_data.mint_b, self.mint_b);
            assert_eq!(escrow_data.receive, amount);
        }

        pub fn assert_ata(&self, owner: &Pubkey, mint: &Pubkey, ata: &Pubkey, amount: u64) {
            let ata_account = self.program.get_account(ata).unwrap();
            let ata_data = spl_token::state::Account::unpack(&ata_account.data).unwrap();

            assert_eq!(ata_data.amount, amount);
            assert_eq!(ata_data.owner, *owner);
            assert_eq!(ata_data.mint, *mint);
        }
    }

    static PROGRAM_ID: Pubkey = crate::ID;

    // Setup function to initialize LiteSVM and create a payer keypair
    // Also loads an account from devnet into the LiteSVM environment (for testing purposes)
    fn setup() -> TestConfig {
        // Initialize LiteSVM and payer
        let mut program = LiteSVM::new();
        let payer = Keypair::new();

        // Airdrop some SOL to the payer keypair
        program
            .airdrop(&payer.pubkey(), 10 * LAMPORTS_PER_SOL)
            .expect("Failed to airdrop SOL to payer");

        // Load program SO file
        let so_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/deploy/anchor_escrow.so");

        let program_data = std::fs::read(so_path).expect("Failed to read program SO file");

        program.add_program(PROGRAM_ID, &program_data);

        // Example on how to Load an account from devnet
        // LiteSVM does not have access to real Solana network data since it does not have network access,
        // so we use an RPC client to fetch account data from devnet
        let rpc_client = RpcClient::new("https://api.devnet.solana.com");
        let account_address =
            Address::from_str("DRYvf71cbF2s5wgaJQvAGkghMkRcp5arvsK2w97vXhi2").unwrap();
        let fetched_account = rpc_client
            .get_account(&account_address)
            .expect("Failed to fetch account from devnet");

        // Set the fetched account in the LiteSVM environment
        // This allows us to simulate interactions with this account during testing
        // program
        //     .set_account(
        //         payer.pubkey(),
        //         Account {
        //             lamports: fetched_account.lamports,
        //             data: fetched_account.data,
        //             owner: Pubkey::from(fetched_account.owner.to_bytes()),
        //             executable: fetched_account.executable,
        //             rent_epoch: fetched_account.rent_epoch,
        //         },
        //     )
        //     .unwrap();

        // msg!("Lamports of fetched account: {}", fetched_account.lamports);

        // // Return the LiteSVM instance and payer keypair
        // (program, payer)

        let maker = payer.pubkey();
        let mint_a = CreateMint::new(&mut program, &payer)
            .decimals(6)
            .authority(&maker)
            .send()
            .unwrap();
        msg!("Mint A: {}\n", mint_a);

        let mint_b = CreateMint::new(&mut program, &payer)
            .decimals(6)
            .authority(&maker)
            .send()
            .unwrap();
        msg!("Mint B: {}\n", mint_b);

        // Create the maker's associated token account for Mint A
        // This is done using litesvm-token's CreateAssociatedTokenAccount utility
        let maker_ata_a = CreateAssociatedTokenAccount::new(&mut program, &payer, &mint_a)
            .owner(&maker)
            .send()
            .unwrap();
        msg!("Maker ATA A: {}\n", maker_ata_a);

        let maker_ata_b = CreateAssociatedTokenAccount::new(&mut program, &payer, &mint_b)
            .owner(&maker)
            .send()
            .unwrap();
        msg!("Maker ATA B: {}\n", maker_ata_b);

        let escrow = Pubkey::find_program_address(
            &[b"escrow", maker.as_ref(), &123u64.to_le_bytes()],
            &PROGRAM_ID,
        )
        .0;
        msg!("Escrow PDA: {}\n", escrow);

        let vault = associated_token::get_associated_token_address(&escrow, &mint_a);
        msg!("Vault PDA: {}\n", vault);

        // Define program IDs for associated token program, token program, and system program
        let associated_token_program = spl_associated_token_account::ID;
        let token_program = TOKEN_PROGRAM_ID;
        let system_program = SYSTEM_PROGRAM_ID;

        // Mint 1,000 tokens (with 6 decimal places) of Mint A to the maker's associated token account
        MintTo::new(&mut program, &payer, &mint_a, &maker_ata_a, 1000000000)
            .send()
            .unwrap();

        let taker = Keypair::new();
        program
            .airdrop(&taker.pubkey(), 10 * LAMPORTS_PER_SOL)
            .expect("Failed to airdrop SOL to taker");

        let taker_ata_a = CreateAssociatedTokenAccount::new(&mut program, &taker, &mint_a)
            .owner(&taker.pubkey())
            .send()
            .unwrap();
        msg!("Taker ATA A: {}\n", taker_ata_a);

        let taker_ata_b = CreateAssociatedTokenAccount::new(&mut program, &taker, &mint_b)
            .owner(&taker.pubkey())
            .send()
            .unwrap();
        msg!("Taker ATA B: {}\n", taker_ata_b);

        MintTo::new(&mut program, &payer, &mint_b, &taker_ata_b, 1000000000)
            .send()
            .unwrap();

        TestConfig {
            program,
            payer,
            maker,
            taker,
            mint_a,
            mint_b,
            maker_ata_a,
            maker_ata_b,
            taker_ata_a,
            taker_ata_b,
            escrow,
            vault,
            associated_token_program,
            token_program,
            system_program,
        }
    }

    #[test]
    fn test_make() {
        let mut test_config: TestConfig = setup();
        let deposit_amount = 10;
        let receive_amount = 10;
        test_config.make(123u64, deposit_amount, receive_amount);

        test_config.assert_vault(deposit_amount);
        test_config.assert_ata(
            &test_config.maker,
            &test_config.mint_a,
            &test_config.maker_ata_a,
            999999990,
        );
        test_config.assert_escrow(deposit_amount);
    }

    #[test]
    fn test_take() {
        let mut test_config: TestConfig = setup();
        let deposit_amount = 10;
        let receive_amount = 10;

        test_config.make(123u64, deposit_amount, receive_amount);

        test_config.assert_vault(deposit_amount);
        test_config.assert_ata(
            &test_config.maker,
            &test_config.mint_a,
            &test_config.maker_ata_a,
            999999990,
        );
        test_config.assert_escrow(deposit_amount);
        test_config.take();
        msg!("From Taker_Ata_a -> Maker_Ata_a");
        test_config.assert_ata(
            &test_config.taker.pubkey(),
            &test_config.mint_b,
            &test_config.taker_ata_b,
            999999990,
        );
        test_config.assert_ata(
            &test_config.taker.pubkey(),
            &test_config.mint_a,
            &test_config.taker_ata_a,
            10,
        );
        test_config.assert_ata(
            &test_config.maker,
            &test_config.mint_b,
            &test_config.maker_ata_b,
            10,
        );
    }

    #[test]
    #[test]
    fn test_refund() {
        let mut test_config: TestConfig = setup();
        let deposit_amount = 10;
        let receive_amount = 10;
        test_config.make(123u64, deposit_amount, receive_amount);
        test_config.refund();
        test_config.assert_ata(
            &test_config.maker,
            &test_config.mint_a,
            &test_config.maker_ata_a,
            1000000000,
        );
    }
}
