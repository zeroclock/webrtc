#[cfg(test)]
mod context_test {
    use crate::{
        context, context::Context, key_derivation::*, protection_profile::ProtectionProfile,
    };

    use util::Error;

    const CIPHER_CONTEXT_ALGO: ProtectionProfile = ProtectionProfile::AES128CMHMACSHA1_80;
    const DEFAULT_SSRC: u32 = 0;

    #[test]
    fn test_context_roc() -> Result<(), Error> {
        let key_len = CIPHER_CONTEXT_ALGO.key_len()?;
        let salt_len = CIPHER_CONTEXT_ALGO.salt_len()?;

        let mut c = Context::new(
            &vec![0; key_len],
            &vec![0; salt_len],
            CIPHER_CONTEXT_ALGO,
            None,
            None,
        )?;

        let roc = c.get_roc(123);
        assert!(roc.is_none(), "ROC must return None for unused SSRC");

        c.set_roc(123, 100);
        let roc = c.get_roc(123);
        if let Some(r) = roc {
            assert_eq!(r, 100, "ROC is set to 100, but returned {}", r)
        } else {
            assert!(false, "ROC must return value for used SSRC");
        }

        Ok(())
    }

    #[test]
    fn test_context_index() -> Result<(), Error> {
        let key_len = CIPHER_CONTEXT_ALGO.key_len()?;
        let salt_len = CIPHER_CONTEXT_ALGO.salt_len()?;

        let mut c = Context::new(
            &vec![0; key_len],
            &vec![0; salt_len],
            CIPHER_CONTEXT_ALGO,
            None,
            None,
        )?;

        let index = c.get_index(123);
        assert!(index.is_none(), "Index must return None for unused SSRC");

        c.set_index(123, 100);
        let index = c.get_index(123);
        if let Some(i) = index {
            assert_eq!(i, 100, "Index is set to 100, but returned {}", i);
        } else {
            assert!(false, "Index must return true for used SSRC")
        }

        Ok(())
    }

    #[test]
    fn test_key_len() -> Result<(), Error> {
        let key_len = CIPHER_CONTEXT_ALGO.key_len()?;
        let salt_len = CIPHER_CONTEXT_ALGO.salt_len()?;

        let result = Context::new(&vec![], &vec![0; salt_len], CIPHER_CONTEXT_ALGO, None, None);
        assert!(result.is_err(), "CreateContext accepted a 0 length key");

        let result = Context::new(&vec![0; key_len], &vec![], CIPHER_CONTEXT_ALGO, None, None);
        assert!(result.is_err(), "CreateContext accepted a 0 length salt");

        let result = Context::new(
            &vec![0; key_len],
            &vec![0; salt_len],
            CIPHER_CONTEXT_ALGO,
            None,
            None,
        );
        assert!(
            result.is_ok(),
            "CreateContext failed with a valid length key and salt"
        );

        Ok(())
    }

    #[test]
    fn test_valid_packet_counter() -> Result<(), Error> {
        let master_key = vec![
            0x0d, 0xcd, 0x21, 0x3e, 0x4c, 0xbc, 0xf2, 0x8f, 0x01, 0x7f, 0x69, 0x94, 0x40, 0x1e,
            0x28, 0x89,
        ];
        let master_salt = vec![
            0x62, 0x77, 0x60, 0x38, 0xc0, 0x6d, 0xc9, 0x41, 0x9f, 0x6d, 0xd9, 0x43, 0x3e, 0x7c,
        ];

        let srtp_session_salt = aes_cm_key_derivation(
            context::LABEL_SRTP_SALT,
            &master_key,
            &master_salt,
            0,
            master_salt.len(),
        )?;

        let s = context::SrtpSsrcState {
            ssrc: 4160032510,
            ..Default::default()
        };
        let expected_counter = vec![
            0xcf, 0x90, 0x1e, 0xa5, 0xda, 0xd3, 0x2c, 0x15, 0x00, 0xa2, 0x24, 0xae, 0xae, 0xaf,
            0x00, 0x00,
        ];
        let counter = generate_counter(32846, s.rollover_counter, s.ssrc, &srtp_session_salt)?;
        assert_eq!(
            counter, expected_counter,
            "Session Key {:?} does not match expected {:?}",
            counter, expected_counter,
        );

        Ok(())
    }

    #[test]
    fn test_rollover_count() -> Result<(), Error> {
        let mut s = context::SrtpSsrcState {
            ssrc: DEFAULT_SSRC,
            ..Default::default()
        };

        // Set initial seqnum
        let roc = s.next_rollover_count(65530);
        assert_eq!(roc, 0, "Initial rolloverCounter must be 0");
        s.update_rollover_count(65530);

        // Invalid packets never update ROC
        s.next_rollover_count(0);
        s.next_rollover_count(0x4000);
        s.next_rollover_count(0x8000);
        s.next_rollover_count(0xFFFF);
        s.next_rollover_count(0);

        // We rolled over to 0
        let roc = s.next_rollover_count(0);
        assert_eq!(roc, 1, "rolloverCounter was not updated after it crossed 0");
        s.update_rollover_count(0);

        let roc = s.next_rollover_count(65530);
        assert_eq!(
            roc, 0,
            "rolloverCounter was not updated when it rolled back, failed to handle out of order"
        );
        s.update_rollover_count(65530);

        let roc = s.next_rollover_count(5);
        assert_eq!(
            roc, 1,
            "rolloverCounter was not updated when it rolled over initial, to handle out of order"
        );
        s.update_rollover_count(5);

        s.next_rollover_count(6);
        s.update_rollover_count(6);

        s.next_rollover_count(7);
        s.update_rollover_count(7);

        let roc = s.next_rollover_count(8);
        assert_eq!(
            roc, 1,
            "rolloverCounter was improperly updated for non-significant packets"
        );
        s.update_rollover_count(8);

        // valid packets never update ROC
        let roc = s.next_rollover_count(0x4000);
        assert_eq!(
            roc, 1,
            "rolloverCounter was improperly updated for non-significant packets"
        );
        s.update_rollover_count(0x4000);

        let roc = s.next_rollover_count(0x8000);
        assert_eq!(
            roc, 1,
            "rolloverCounter was improperly updated for non-significant packets"
        );
        s.update_rollover_count(0x8000);

        let roc = s.next_rollover_count(0xFFFF);
        assert_eq!(
            roc, 1,
            "rolloverCounter was improperly updated for non-significant packets"
        );
        s.update_rollover_count(0xFFFF);

        let roc = s.next_rollover_count(0);
        assert_eq!(
            roc, 2,
            "rolloverCounter must be incremented after wrapping, got {}",
            roc
        );

        Ok(())
    }
}
