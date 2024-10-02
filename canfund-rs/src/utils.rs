use crate::errors::Error;

/// Converts the cycles from `candid::Nat` to `u128`.
pub fn cycles_nat_to_u128(cycles: candid::Nat) -> Result<u128, Error> {
    let cycles_text = cycles.0.to_string();

    match cycles.0.try_into() {
        Ok(cycles) => Ok(cycles),
        Err(_) => Err(Error::FailedCyclesConversion {
            cycles: cycles_text,
        }),
    }
}

/// Converts the cycles from `String` to `u128`.
pub fn cycles_str_to_u128(cycles: &str) -> Result<u128, Error> {
    match cycles.parse::<u128>() {
        Ok(cycles) => Ok(cycles),
        Err(_) => Err(Error::FailedCyclesConversion {
            cycles: cycles.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_bigint::BigUint;

    #[test]
    fn test_cycles_nat_to_u128() {
        let cycles = candid::Nat(BigUint::from(100u32));
        assert_eq!(cycles_nat_to_u128(cycles).unwrap(), 100);
    }

    #[test]
    fn test_cycles_str_to_u128() {
        assert_eq!(cycles_str_to_u128("100").unwrap(), 100);
    }

    #[test]
    fn test_cycles_str_to_u128_invalid() {
        assert_eq!(
            cycles_str_to_u128("invalid").unwrap_err(),
            Error::FailedCyclesConversion {
                cycles: "invalid".to_string(),
            }
        );
    }

    #[test]
    fn test_cycles_str_to_u128_empty() {
        assert_eq!(
            cycles_str_to_u128("").unwrap_err(),
            Error::FailedCyclesConversion {
                cycles: "".to_string(),
            }
        );
    }
}
