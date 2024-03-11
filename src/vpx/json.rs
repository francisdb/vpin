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
        // we seem to be getting other NaN values than f32::NAN
        let bytes = value.to_le_bytes();
        let hex_string = hex::encode(bytes);
        let nan = format!("NaN|{}", &hex_string);
        serializer.serialize_str(&nan)
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
            "inf" => Ok(f32::INFINITY),
            "-inf" => Ok(f32::NEG_INFINITY),
            other => {
                if other.starts_with("nan|") {
                    let hex_string = &other[4..];
                    let bytes = hex::decode(hex_string)
                        .map_err(|e| serde::de::Error::custom(e.to_string()))?;
                    let mut array = [0; 4];
                    array.copy_from_slice(&bytes);
                    let f = f32::from_le_bytes(array);
                    Ok(f)
                } else {
                    Err(serde::de::Error::custom(format!(
                        r#"expected "NaN|########", "Inf" or "-Inf", found {}"#,
                        other
                    )))
                }
            }
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
    }

    #[test]
    fn test_f32_nan_inf() {
        let f32_num = F32Test { num: 1.0 };
        let f32_inf = F32Test { num: f32::INFINITY };
        let f32_n_inf = F32Test {
            num: f32::NEG_INFINITY,
        };
        let f32_nan = F32Test { num: f32::NAN };
        let f32_other_nan = F32Test {
            num: f32::from_le_bytes([0xff, 0xff, 0xff, 0xff]),
        };

        let json_num = json!({"num": 1.0});
        let json_inf = json!({"num": "Inf"});
        let json_n_inf = json!({"num": "-Inf"});
        let json_nan = json!({"num": "NaN|0000c07f"});
        let json_other_nan = json!({"num": "NaN|ffffffff"});

        assert_eq!(serde_json::to_value(&f32_num).unwrap(), json_num);
        assert_eq!(serde_json::to_value(&f32_inf).unwrap(), json_inf);
        assert_eq!(serde_json::to_value(&f32_n_inf).unwrap(), json_n_inf);
        assert_eq!(serde_json::to_value(&f32_nan).unwrap(), json_nan);
        assert_eq!(
            serde_json::to_value(&f32_other_nan).unwrap(),
            json_other_nan
        );

        assert_eq!(
            serde_json::from_value::<F32Test>(json_num).unwrap(),
            f32_num
        );
        assert_eq!(
            serde_json::from_value::<F32Test>(json_inf).unwrap(),
            f32_inf
        );
        assert_eq!(
            serde_json::from_value::<F32Test>(json_n_inf).unwrap(),
            f32_n_inf
        );
        // can't compare nan values
        // assert_eq!(
        //     serde_json::from_value::<F32Test>(json_nan).unwrap(),
        //     f32_nan
        // );
        // assert_eq!(
        //     serde_json::from_value::<F32Test>(json_other_nan).unwrap(),
        //     f32_other_nan
        // );
    }
}
