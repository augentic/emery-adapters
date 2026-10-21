//! Typed display values: what a `format:` on a bound text makes of its field.
//!
//! The only copy a design file carries is what was drawn, so locale data (weekday names, unit
//! symbols, unit long names) exists only for the one value the designer happened to draw. A
//! `format:` says what kind of value a text shows instead: the field holds a typed value
//! ([`Display`]) rather than words, and the view formats it with the browser's `Intl` formatters.
//! A field bound as two kinds of value is a [`crate::contract::Conflict`].

/// What a measurement measures. Each has a closed unit set.
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

impl Quantity {
    /// Every quantity.
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

    /// The name of a value with its unit.
    #[must_use]
    pub const fn measure_type(self) -> &'static str {
        match self {
            Self::Temperature => "Temperature",
            Self::Speed => "Speed",
            Self::Pressure => "Pressure",
            Self::Distance => "Distance",
        }
    }

    /// The name of a unit alone.
    #[must_use]
    pub const fn unit_type(self) -> &'static str {
        match self {
            Self::Temperature => "TemperatureUnit",
            Self::Speed => "SpeedUnit",
            Self::Pressure => "PressureUnit",
            Self::Distance => "DistanceUnit",
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
    /// The style a composition spells, or `None` outside the closed list validation enforces.
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

    /// The kind of value a field shown this way holds, which is what a field is typed by.
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
}

/// The typed value a formatted field holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Display {
    /// `Moment`: an instant, the offset to show it at, and whether it has a time of day.
    Moment,
    /// `Temperature`, `Speed`, …: a value and its unit.
    Measure(Quantity),
    /// `TemperatureUnit`, `SpeedUnit`, …: a unit alone.
    Unit(Quantity),
    /// `Number`: a plain number.
    Number,
}

impl Display {
    /// Its name, for a message that quotes it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Moment => "Moment",
            Self::Measure(quantity) => quantity.measure_type(),
            Self::Unit(quantity) => quantity.unit_type(),
            Self::Number => "Number",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_admitted_style_parses_and_spells_back_the_same() {
        for style in crate::spec::composition::formats::FORMAT_STYLES {
            let format = Format::parse(style).unwrap_or_else(|| panic!("`{style}` parses"));
            assert_eq!(format.style(), style);
        }
        assert_eq!(Format::parse("weekdays"), None);
        assert_eq!(Format::parse("volume-unit"), None);
    }
}
