//! Typed display values: what a `format:` on a bound text makes of its field, and how each shell
//! formats one with the device locale.
//!
//! The only copy a design file carries is what was drawn, so locale data — weekday names, unit
//! symbols, unit long names — existed only for the one value the designer happened to draw
//! (`handoff-copy-source.md`). A `format:` says what kind of value a text shows instead. The
//! contract then carries a typed value ([`Display`]) rather than a `String`, the core fills it with
//! no words at all, and each view formats it with the platform's own formatter:
//!
//! | Shell | Formatter |
//! | --- | --- |
//! | `SwiftUI` | `Date.FormatStyle`, `Measurement.FormatStyle`, `MeasurementFormatter` |
//! | Compose | `android.icu` `DateFormat` skeletons, `MeasureFormat`, `NumberFormat` |
//! | Dioxus web | `Intl.DateTimeFormat`, `Intl.NumberFormat` |
//! | Dioxus desktop and mobile | a small formatter, fixed to English |
//!
//! Nothing here is a designer knob. The decimals a measurement shows are a per-unit table
//! ([`Unit::decimals`]), the same on every shell.
//!
//! Copied from vx-emit's `format.rs`: the vocabulary and the typed values alone. The Swift, Kotlin
//! and Dioxus formatter files and the contract's Rust declarations did not come across.

/// What a measurement measures. Each has a closed unit set, and a struct and an enum of its own in
/// the contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Quantity {
    /// `Temperature` / `TemperatureUnit`.
    Temperature,
    /// `Speed` / `SpeedUnit`.
    Speed,
    /// `Pressure` / `PressureUnit`.
    Pressure,
    /// `Distance` / `DistanceUnit`.
    Distance,
}

/// One unit of a [`Quantity`], and how every shell spells it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unit {
    /// The contract's Rust variant.
    pub variant: &'static str,

    /// The CLDR root symbol: what the web shows for a unit `Intl` does not sanction, and what the
    /// English formatter shows everywhere.
    pub symbol: &'static str,

    /// The English long name, for the Dioxus formatter that has no locale data.
    pub english: &'static str,

    /// The most fraction digits a value in this unit shows, on every shell.
    pub decimals: u8,

    /// The Foundation `Dimension` member, after its type.
    pub foundation: &'static str,

    /// The `android.icu.util.MeasureUnit` constant.
    pub icu: &'static str,

    /// The ECMA-402 sanctioned unit identifier, when there is one.
    pub web: Option<&'static str>,
}

const fn unit(
    variant: &'static str, symbol: &'static str, english: &'static str, decimals: u8,
    foundation: &'static str, icu: &'static str, web: Option<&'static str>,
) -> Unit {
    Unit {
        variant,
        symbol,
        english,
        decimals,
        foundation,
        icu,
        web,
    }
}

const TEMPERATURE: [Unit; 2] = [
    unit("Celsius", "°C", "Degrees Celsius", 0, "celsius", "CELSIUS", Some("celsius")),
    unit(
        "Fahrenheit",
        "°F",
        "Degrees Fahrenheit",
        0,
        "fahrenheit",
        "FAHRENHEIT",
        Some("fahrenheit"),
    ),
];

const SPEED: [Unit; 4] = [
    unit(
        "KilometrePerHour",
        "km/h",
        "Kilometres per hour",
        0,
        "kilometersPerHour",
        "KILOMETER_PER_HOUR",
        Some("kilometer-per-hour"),
    ),
    unit(
        "MilePerHour",
        "mph",
        "Miles per hour",
        0,
        "milesPerHour",
        "MILE_PER_HOUR",
        Some("mile-per-hour"),
    ),
    unit(
        "MetrePerSecond",
        "m/s",
        "Metres per second",
        0,
        "metersPerSecond",
        "METER_PER_SECOND",
        Some("meter-per-second"),
    ),
    unit("Knot", "kn", "Knots", 0, "knots", "KNOT", None),
];

const PRESSURE: [Unit; 4] = [
    unit("Hectopascal", "hPa", "Hectopascals", 0, "hectopascals", "HECTOPASCAL", None),
    unit("Millibar", "mbar", "Millibars", 0, "millibars", "MILLIBAR", None),
    unit("InchOfMercury", "inHg", "Inches of mercury", 2, "inchesOfMercury", "INCH_HG", None),
    unit(
        "MillimetreOfMercury",
        "mmHg",
        "Millimetres of mercury",
        0,
        "millimetersOfMercury",
        "MILLIMETER_OF_MERCURY",
        None,
    ),
];

const DISTANCE: [Unit; 4] = [
    unit("Kilometre", "km", "Kilometres", 1, "kilometers", "KILOMETER", Some("kilometer")),
    unit("Metre", "m", "Metres", 0, "meters", "METER", Some("meter")),
    unit("Mile", "mi", "Miles", 0, "miles", "MILE", Some("mile")),
    unit("Foot", "ft", "Feet", 0, "feet", "FOOT", Some("foot")),
];

/// The most fraction digits a `number` shows. A percent shows none.
pub const NUMBER_DECIMALS: u8 = 2;

impl Quantity {
    /// Every quantity, in the order the contract declares their types.
    pub const ALL: [Self; 4] = [Self::Temperature, Self::Speed, Self::Pressure, Self::Distance];

    /// The style prefix, which is also the quantity's name in prose.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Temperature => "temperature",
            Self::Speed => "speed",
            Self::Pressure => "pressure",
            Self::Distance => "distance",
        }
    }

    /// The contract struct holding a value and its unit.
    #[must_use]
    pub const fn measure_type(self) -> &'static str {
        match self {
            Self::Temperature => "Temperature",
            Self::Speed => "Speed",
            Self::Pressure => "Pressure",
            Self::Distance => "Distance",
        }
    }

    /// The contract enum of its units.
    #[must_use]
    pub const fn unit_type(self) -> &'static str {
        match self {
            Self::Temperature => "TemperatureUnit",
            Self::Speed => "SpeedUnit",
            Self::Pressure => "PressureUnit",
            Self::Distance => "DistanceUnit",
        }
    }

    /// Its closed unit set, the first the default.
    #[must_use]
    pub const fn units(self) -> &'static [Unit] {
        match self {
            Self::Temperature => &TEMPERATURE,
            Self::Speed => &SPEED,
            Self::Pressure => &PRESSURE,
            Self::Distance => &DISTANCE,
        }
    }

    fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|quantity| quantity.name() == name)
    }
}

/// How a bound text shows its value: one of the closed `format:` styles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Format {
    /// `weekday`: the day's name.
    Weekday,
    /// `time`: the time of day, 12- or 24-hour as the locale says.
    Time,
    /// `date`: the calendar date.
    Date,
    /// `date-time`: the date, and the time when the value has one.
    DateTime,
    /// `temperature`, `speed`, `pressure`, `distance`: a value with its unit.
    Measure(Quantity),
    /// `<quantity>-unit`: the unit's symbol.
    Unit(Quantity),
    /// `<quantity>-unit-name`: the unit's long name.
    UnitName(Quantity),
    /// `number`.
    Number,
    /// `percent`, from percentage points.
    Percent,
}

impl Format {
    /// The style a composition spells, or `None` outside the closed list `vx spec` enforces.
    #[must_use]
    pub fn parse(style: &str) -> Option<Self> {
        Some(match style {
            "weekday" => Self::Weekday,
            "time" => Self::Time,
            "date" => Self::Date,
            "date-time" => Self::DateTime,
            "number" => Self::Number,
            "percent" => Self::Percent,
            _ => {
                if let Some(quantity) = style.strip_suffix("-unit-name") {
                    Self::UnitName(Quantity::parse(quantity)?)
                } else if let Some(quantity) = style.strip_suffix("-unit") {
                    Self::Unit(Quantity::parse(quantity)?)
                } else {
                    Self::Measure(Quantity::parse(style)?)
                }
            }
        })
    }

    /// The style as the composition spells it.
    #[must_use]
    pub fn style(self) -> String {
        match self {
            Self::Weekday => "weekday".to_owned(),
            Self::Time => "time".to_owned(),
            Self::Date => "date".to_owned(),
            Self::DateTime => "date-time".to_owned(),
            Self::Number => "number".to_owned(),
            Self::Percent => "percent".to_owned(),
            Self::Measure(quantity) => quantity.name().to_owned(),
            Self::Unit(quantity) => format!("{}-unit", quantity.name()),
            Self::UnitName(quantity) => format!("{}-unit-name", quantity.name()),
        }
    }

    /// The kind of value a field shown this way holds, which is what the contract types it by.
    ///
    /// Styles of one kind share it: a moment shown as its `weekday` here and its `time` there is
    /// one field, as is a unit shown as its symbol and its long name.
    #[must_use]
    pub const fn display(self) -> Display {
        match self {
            Self::Weekday | Self::Time | Self::Date | Self::DateTime => Display::Moment,
            Self::Measure(quantity) => Display::Measure(quantity),
            Self::Unit(quantity) | Self::UnitName(quantity) => Display::Unit(quantity),
            Self::Number | Self::Percent => Display::Number,
        }
    }

    /// The helper function formatting it, in lower camel case for Swift and Kotlin.
    #[must_use]
    pub fn camel(self) -> String {
        crate::naming::camel(&self.style())
    }

    /// The helper function formatting it, in snake case for Rust.
    #[must_use]
    pub fn snake(self) -> String {
        self.style().replace('-', "_")
    }
}

/// The typed value a formatted field holds in the contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Display {
    /// `Moment`: an instant, the offset to show it at, and whether it has a time of day.
    Moment,
    /// `Temperature`, `Speed`, …: a value and its unit.
    Measure(Quantity),
    /// `TemperatureUnit`, `SpeedUnit`, …: a unit alone.
    Unit(Quantity),
    /// `f64`.
    Number,
}

impl Display {
    /// How Rust spells it.
    #[must_use]
    pub const fn rust(self) -> &'static str {
        match self {
            Self::Moment => "Moment",
            Self::Measure(quantity) => quantity.measure_type(),
            Self::Unit(quantity) => quantity.unit_type(),
            Self::Number => "f64",
        }
    }

    /// How Swift spells it.
    #[must_use]
    pub const fn swift(self) -> &'static str {
        match self {
            Self::Number => "Double",
            _ => self.rust(),
        }
    }

    /// How Kotlin spells it.
    #[must_use]
    pub const fn kotlin(self) -> &'static str {
        self.swift()
    }

    /// What the value means, for the field's doc comment.
    #[must_use]
    pub const fn unit(self) -> &'static str {
        match self {
            Self::Moment => {
                "A moment the view formats with the device locale: epoch milliseconds, the UTC \
                 offset to show it at (`None` is the device's zone), and whether it has a time of \
                 day."
            }
            Self::Measure(_) => {
                "A value in the unit beside it, which the view formats with the device locale. \
                 Never converted: the unit is the core's decision."
            }
            Self::Unit(_) => "A unit, which the view names with the device locale.",
            Self::Number => {
                "A number the view formats with the device locale. Where it is shown as a \
                 percent, it is in percentage points: 42.0 is 42%."
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_style_vx_spec_admits_parses_and_spells_back_the_same() {
        for style in crate::spec::composition::formats::FORMAT_STYLES {
            let format = Format::parse(style).unwrap_or_else(|| panic!("`{style}` parses"));
            assert_eq!(format.style(), style);
        }
        assert_eq!(Format::parse("weekdays"), None);
        assert_eq!(Format::parse("volume-unit"), None);
    }

    #[test]
    fn a_moment_style_is_a_moment_whatever_part_of_it_shows() {
        for style in ["weekday", "time", "date", "date-time"] {
            let format = Format::parse(style).expect("parses");
            assert_eq!(format.display(), Display::Moment);
            assert_eq!(format.display().rust(), "Moment");
            assert_eq!(format.display().swift(), "Moment");
            assert_eq!(format.display().kotlin(), "Moment");
        }
    }

    #[test]
    fn a_measurement_is_its_quantity_and_a_unit_alone_is_its_unit_enum() {
        let speed = Format::parse("speed").expect("parses");
        assert_eq!(speed.display(), Display::Measure(Quantity::Speed));
        assert_eq!(speed.display().rust(), "Speed");
        for style in ["speed-unit", "speed-unit-name"] {
            let format = Format::parse(style).expect("parses");
            assert_eq!(format.display(), Display::Unit(Quantity::Speed));
            assert_eq!(format.display().rust(), "SpeedUnit");
        }
        assert_eq!(
            Format::parse("pressure-unit-name").expect("parses").display().kotlin(),
            "PressureUnit"
        );
    }

    #[test]
    fn a_number_and_a_percent_are_both_a_plain_number() {
        for style in ["number", "percent"] {
            let display = Format::parse(style).expect("parses").display();
            assert_eq!(display, Display::Number);
            assert_eq!(
                (display.rust(), display.swift(), display.kotlin()),
                ("f64", "Double", "Double")
            );
        }
    }

    #[test]
    fn the_unit_sets_are_the_decided_ones_with_their_decimals() {
        let spelled = |quantity: Quantity| -> Vec<(&str, u8)> {
            quantity.units().iter().map(|unit| (unit.symbol, unit.decimals)).collect()
        };
        assert_eq!(spelled(Quantity::Temperature), [("°C", 0), ("°F", 0)]);
        assert_eq!(spelled(Quantity::Speed), [("km/h", 0), ("mph", 0), ("m/s", 0), ("kn", 0)]);
        assert_eq!(
            spelled(Quantity::Pressure),
            [("hPa", 0), ("mbar", 0), ("inHg", 2), ("mmHg", 0)]
        );
        assert_eq!(spelled(Quantity::Distance), [("km", 1), ("m", 0), ("mi", 0), ("ft", 0)]);
        assert_eq!(NUMBER_DECIMALS, 2);
    }

    #[test]
    fn the_web_sanctions_neither_the_knot_nor_any_pressure_unit() {
        let unsanctioned: Vec<&str> = Quantity::ALL
            .iter()
            .flat_map(|quantity| quantity.units())
            .filter(|unit| unit.web.is_none())
            .map(|unit| unit.symbol)
            .collect();
        assert_eq!(unsanctioned, ["kn", "hPa", "mbar", "inHg", "mmHg"]);
    }
}
