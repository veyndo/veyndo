//! Property tests for money, rounding, discounts and the proved commerce sums: generated inputs against independent integer models.
use proptest::prelude::*;
use veyndo_pricing::binary::Binary;
use veyndo_pricing::decimal::Decimal;
use veyndo_pricing::discount::allocate;
use veyndo_pricing::money::{Currency, Minor, Money};
use veyndo_pricing::pricing::{PriceInput, calculate, math_round};
use veyndo_pricing::quote::QuoteTotals;
use veyndo_pricing::verified_kernel::{amount_sum, discount_cap, net_amount};

fn currency(scale: u8) -> Currency {
    Currency::new("XTS", scale).unwrap()
}

/// Decimal text of `minor` at `scale`, written independently of `Money::decimal`.
fn text(minor: i128, scale: u32) -> String {
    let sign = if minor < 0 { "-" } else { "" };
    let factor = 10_i128.pow(scale);
    let absolute = minor.abs();
    if scale == 0 {
        format!("{sign}{absolute}")
    } else {
        format!(
            "{sign}{}.{:0width$}",
            absolute / factor,
            absolute % factor,
            width = scale as usize
        )
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Every supported scale is admitted, every larger one refused (Lean: `currency_scale_admissible`).
    #[test]
    fn currency_scale_is_bounded(scale in 0_u8..=255) {
        prop_assert_eq!(Currency::new("XTS", scale).is_ok(), scale <= 6);
    }

    /// Decimal text and minor units are one exact bijection for every amount and scale.
    #[test]
    fn decimal_text_roundtrips_exactly(minor in any::<i64>(), scale in 0_u8..=6) {
        let money = Money::new(minor, currency(scale));
        let decimal = money.decimal();
        prop_assert_eq!(&decimal, &text(i128::from(minor), u32::from(scale)));
        prop_assert_eq!(Money::parse(&decimal, currency(scale)).unwrap(), money);
    }

    /// Parsing never truncates: any digit count other than the scale is refused.
    #[test]
    fn parsing_refuses_a_wrong_scale(major in 0_u32..1_000_000, digits in 0_usize..9, scale in 0_u8..=6) {
        let fraction = "7".repeat(digits);
        let value = if digits == 0 { major.to_string() } else { format!("{major}.{fraction}") };
        prop_assert_eq!(Money::parse(&value, currency(scale)).is_ok(), digits == usize::from(scale));
    }

    /// Rounding to the minor unit is half away from zero, decided on the decimal digits.
    #[test]
    fn rounding_is_half_away_from_zero(
        major in 0_u64..1_000_000_000,
        fraction in "[0-9]{0,12}",
        negative in any::<bool>(),
        scale in 0_u8..=6,
    ) {
        let value = format!("{}{major}{}", if negative { "-" } else { "" },
            if fraction.is_empty() { String::new() } else { format!(".{fraction}") });
        let rounded = Money::rounded(&value, currency(scale)).unwrap();
        let scale = usize::from(scale);
        let kept: String = format!("{fraction:0<scale$}").chars().take(scale).collect();
        let mut expected = i128::from(major) * 10_i128.pow(scale as u32)
            + if kept.is_empty() { 0 } else { kept.parse::<i128>().unwrap() };
        if fraction.as_bytes().get(scale).is_some_and(|d| *d >= b'5') {
            expected += 1;
        }
        if negative { expected = -expected; }
        prop_assert_eq!(i128::from(rounded.minor()), expected);
    }

    /// Scaling: an amount already exact at the scale is unchanged by rounding, and the
    /// rounding error of any longer decimal stays within half a minor unit.
    #[test]
    fn rounding_error_is_at_most_half_a_minor_unit(
        minor in -1_000_000_000_000_i64..1_000_000_000_000,
        extra in "[0-9]{1,6}",
        scale in 0_u8..=6,
    ) {
        let exact = Money::new(minor, currency(scale));
        prop_assert_eq!(Money::rounded(&exact.decimal(), currency(scale)).unwrap(), exact.clone());
        let longer = if scale == 0 { format!("{}.{extra}", exact.decimal()) } else { format!("{}{extra}", exact.decimal()) };
        let rounded = Money::rounded(&longer, currency(scale)).unwrap();
        // Distance in units of 10^-(scale + extra digits), compared without floating point.
        let unit = 10_i128.pow(extra.len() as u32);
        let tail = extra.parse::<i128>().unwrap() * if minor < 0 || longer.starts_with('-') { -1 } else { 1 };
        let distance = (i128::from(rounded.minor()) * unit - (i128::from(minor) * unit + tail)).abs();
        prop_assert!(2 * distance <= unit, "{longer} -> {}", rounded.decimal());
    }

    /// Addition is exact or refused: never a silent wrap and never across currencies.
    #[test]
    fn checked_addition_is_exact_or_refused(a in any::<i64>(), b in any::<i64>(), scale in 0_u8..=6) {
        let left = Money::new(a, currency(scale));
        let right = Money::new(b, currency(scale));
        let sum = left.checked_add(&right);
        match a.checked_add(b) {
            Some(expected) => prop_assert_eq!(sum.unwrap().minor(), expected),
            None => prop_assert!(sum.is_err()),
        }
        prop_assert_eq!(left.checked_add(&right).ok(), right.checked_add(&left).ok());
        let other = Money::new(b, Currency::new("XTS", (scale + 1) % 7).unwrap());
        prop_assert!(left.checked_add(&other).is_err());
    }

    /// Lean `discount_bounded`, `discount_requested`, `discount_idempotent`, `discount_monotone_requested`.
    #[test]
    fn the_discount_cap_stays_within_its_base(total in any::<u64>(), first in any::<u64>(), second in any::<u64>()) {
        let cap = discount_cap(total, first);
        prop_assert!(cap <= total && cap <= first);
        prop_assert_eq!(cap, total.min(first));
        prop_assert_eq!(discount_cap(total, cap), cap);
        let (low, high) = (first.min(second), first.max(second));
        prop_assert!(discount_cap(total, low) <= discount_cap(total, high));
    }

    /// The allocated basket discount equals min(goods, requested), no line goes below zero
    /// or above its own total, and lines without positive value keep their amount.
    #[test]
    fn discount_allocation_conserves_cents(
        lines in prop::collection::vec(-10_000_i64..1_000_000, 0..12),
        discount in -1_000_i64..20_000_000,
    ) {
        let result = allocate(&lines, discount);
        prop_assert_eq!(result.len(), lines.len());
        let goods: i64 = lines.iter().map(|n| (*n).max(0)).sum();
        let applied: i64 = lines.iter().zip(&result).map(|(a, b)| a - b).sum();
        prop_assert_eq!(applied, if goods == 0 { 0 } else { discount.max(0).min(goods) });
        for (before, after) in lines.iter().zip(&result) {
            if *before <= 0 {
                prop_assert_eq!(after, before);
            } else {
                prop_assert!(*after >= 0 && after <= before);
            }
        }
    }

    /// A larger requested discount never raises any line total.
    #[test]
    fn discount_allocation_is_monotone(
        lines in prop::collection::vec(0_i64..1_000_000, 1..8),
        first in 0_i64..5_000_000,
        second in 0_i64..5_000_000,
    ) {
        let (low, high) = (first.min(second), first.max(second));
        let less = allocate(&lines, low);
        let more = allocate(&lines, high);
        prop_assert!(less.iter().sum::<i64>() >= more.iter().sum::<i64>());
    }

    /// Lean `amount_sum_exact`, `amount_sum_bounded`, `amount_sum_not_below`, `amount_sum_comm`.
    #[test]
    fn proved_sums_are_exact_or_saturate(a in any::<u64>(), b in any::<u64>()) {
        let sum = amount_sum(a, b);
        prop_assert_eq!(sum, amount_sum(b, a));
        prop_assert!(sum >= a && sum >= b);
        prop_assert_eq!(u128::from(sum), (u128::from(a) + u128::from(b)).min(u128::from(u64::MAX)));
    }

    /// Lean `net_not_above_gross`; net plus tax is the gross whenever the tax is within it.
    #[test]
    fn net_plus_tax_is_gross(gross in any::<u64>(), tax in any::<u64>()) {
        let net = net_amount(gross, tax);
        prop_assert!(net <= gross);
        if tax <= gross {
            prop_assert_eq!(net + tax, gross);
        } else {
            prop_assert_eq!(net, 0);
        }
    }

    /// The original's `round` reads a binary64 value as the decimal it was written as: on the
    /// nearest binary value of a decimal it rounds that decimal half away from zero.
    #[test]
    fn math_rounding_reads_the_written_decimal(units in -100_000_000_000_i64..100_000_000_000, digits in 0_u32..=6, decimals in 0_u8..=4) {
        let written = Decimal::new(units.into(), digits).unwrap();
        let rounded = math_round(Binary::from_decimal(written).unwrap(), decimals).unwrap();
        prop_assert_eq!(rounded, written.ratio().rounded(decimals).unwrap());
    }

    /// Quote totals: a net quote's payable total is position plus tax; a gross quote's net
    /// part plus its tax is the position (Lean `net_quote_total`, `gross_quote_total`).
    #[test]
    fn quote_totals_add_up(position in 0_i64..100_000_000, tax_share in 0_i64..=100, net in any::<bool>()) {
        let tax = Minor::new(position * tax_share / 100, 2);
        let position = Minor::new(position, 2);
        let totals = QuoteTotals::new(position, tax, net).unwrap();
        prop_assert_eq!(totals.position_price, position);
        if net {
            prop_assert_eq!(totals.net_price, position);
            prop_assert_eq!(totals.total_price, position.checked_add(tax).unwrap());
        } else {
            prop_assert_eq!(totals.total_price, position);
            prop_assert_eq!(totals.net_price.checked_add(tax).unwrap(), position);
        }
    }

    /// A line price: the total is the unit times the quantity, the tax is never negative and
    /// never above the gross total, and the same input always gives the same price.
    #[test]
    fn a_line_price_is_unit_times_quantity(
        cents in 0_i64..10_000_000,
        quantity in 1_u32..500,
        rate in prop::sample::select(vec!["0", "7", "10", "19", "20", "21", "7.7"]),
        gross in any::<bool>(),
    ) {
        let input = PriceInput {
            price: Binary::from_decimal(Decimal::new(cents.into(), 2).unwrap()).unwrap(),
            quantity,
            tax_rate: Binary::from_decimal(Decimal::parse(rate).unwrap()).unwrap(),
            gross,
            ..PriceInput::default()
        };
        let price = calculate(&input).unwrap();
        prop_assert_eq!(price.total_price, price.unit_price.times(quantity.into()).unwrap());
        prop_assert!(price.tax.minor() >= 0);
        if gross {
            prop_assert!(price.tax.minor() <= price.total_price.minor());
        }
        prop_assert_eq!(calculate(&input).unwrap(), price);
    }
}
