use serde::{Deserialize, Deserializer, Serializer};
use serde_json::Value;

pub(crate) fn serialize_f32_nan_inf_as_string<S>(
    value: &f32,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if value.is_nan() {
        // NaN string
        let nan = "NaN";
        serializer.serialize_str(nan)
    } else if value.is_sign_positive() && value.is_infinite() {
        let inf = "Inf";
        serializer.serialize_str(inf)
    } else if value.is_sign_negative() && value.is_infinite() {
        let n_inf = "-Inf";
        serializer.serialize_str(n_inf)
    } else {
        serializer.serialize_f32(*value)
    }
}

pub(crate) fn deserialize_f32_nan_inf_from_string<'de, D>(deserializer: D) -> Result<f32, D::Error>
where
    D: Deserializer<'de>,
{
    let v = Value::deserialize(deserializer)?;
    match v {
        Value::String(s) => match s.to_lowercase().as_str() {
            "nan" => Ok(f32::NAN),
            "inf" => Ok(f32::INFINITY),
            "-inf" => Ok(f32::NEG_INFINITY),
            other => Err(serde::de::Error::custom(format!(
                r#"expected "NaN", "Inf" or "-Inf", found {}"#,
                other
            ))),
        },
        Value::Number(n) => n
            .as_f64()
            .ok_or_else(|| serde::de::Error::custom("expected f64"))
            .map(|f| f as f32),
        other => Err(serde::de::Error::custom(format!(
            r#"expected number, "NaN", "Inf" or "-Inf", found {}"#,
            other
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde::Serialize;
    use serde_json::json;

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct F32Test {
        #[serde(
            serialize_with = "serialize_f32_nan_inf_as_string",
            deserialize_with = "deserialize_f32_nan_inf_from_string"
        )]
        num: f32,
        #[serde(
            serialize_with = "serialize_f32_nan_inf_as_string",
            deserialize_with = "deserialize_f32_nan_inf_from_string"
        )]
        inf: f32,
        #[serde(
            serialize_with = "serialize_f32_nan_inf_as_string",
            deserialize_with = "deserialize_f32_nan_inf_from_string"
        )]
        n_inf: f32,
        #[serde(
            serialize_with = "serialize_f32_nan_inf_as_string",
            deserialize_with = "deserialize_f32_nan_inf_from_string"
        )]
        nan: f32,
    }

    #[test]
    fn test_f32_nan_inf() {
        let f32_test = F32Test {
            num: 1.0,
            inf: f32::INFINITY,
            n_inf: f32::NEG_INFINITY,
            nan: f32::NAN,
        };
        let json = json!({
            "num": 1.0,
            "inf": "Inf",
            "n_inf": "-Inf",
            "nan": "NaN"
        });
        let serialized = serde_json::to_string(&f32_test).unwrap();
        assert_eq!(serialized, json.to_string());
        let deserialized: F32Test = serde_json::from_str(&json.to_string()).unwrap();
        assert_eq!(deserialized.num, 1.0);
        assert_eq!(deserialized.inf, f32::INFINITY);
        assert_eq!(deserialized.n_inf, f32::NEG_INFINITY);
        assert!(deserialized.nan.is_nan());
    }
}
