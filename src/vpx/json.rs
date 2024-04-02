use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

/// This is a wrapper for f32 that serializes NaN and Inf as strings.
/// This is needed because serde_json does not support NaN and Inf.
/// It specifically handles multiple NaN values by serializing them as "NaN|########".
#[derive(Debug, PartialEq, Clone, Copy)]
pub(crate) struct F32WithNanInf(f32);

impl From<f32> for F32WithNanInf {
    fn from(f: f32) -> Self {
        F32WithNanInf(f)
    }
}

impl From<F32WithNanInf> for f32 {
    fn from(f: F32WithNanInf) -> f32 {
        f.0
    }
}

impl Serialize for F32WithNanInf {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_f32_nan_inf_as_string(&self.0, serializer)
    }
}

impl<'de> Deserialize<'de> for F32WithNanInf {
    fn deserialize<D>(deserializer: D) -> Result<F32WithNanInf, D::Error>
    where
        D: Deserializer<'de>,
    {
        let f = deserialize_f32_nan_inf_from_string(deserializer)?;
        Ok(F32WithNanInf(f))
    }
}

fn serialize_f32_nan_inf_as_string<S>(value: &f32, serializer: S) -> Result<S::Ok, S::Error>
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

fn deserialize_f32_nan_inf_from_string<'de, D>(deserializer: D) -> Result<f32, D::Error>
where
    D: Deserializer<'de>,
{
    let v = Value::deserialize(deserializer)?;
    match v {
        Value::String(s) => match s.to_lowercase().as_str() {
            "inf" => Ok(f32::INFINITY),
            "-inf" => Ok(f32::NEG_INFINITY),
            other => {
                if let Some(hex_string) = other.strip_prefix("nan|") {
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
    use serde_json::json;

    #[test]
    fn test_f32_nan_inf() {
        let f32_num = F32WithNanInf(1.0);
        let f32_inf = F32WithNanInf(f32::INFINITY);
        let f32_n_inf = F32WithNanInf(f32::NEG_INFINITY);
        let f32_nan = F32WithNanInf(f32::NAN);
        let f32_other_nan = F32WithNanInf(f32::from_le_bytes([0xff, 0xff, 0xff, 0xff]));

        let json_num = json!(1.0);
        let json_inf = json!("Inf");
        let json_n_inf = json!("-Inf");
        let json_nan = json!("NaN|0000c07f");
        let json_other_nan = json!("NaN|ffffffff");

        assert_eq!(serde_json::to_value(f32_num).unwrap(), json_num);
        assert_eq!(serde_json::to_value(f32_inf).unwrap(), json_inf);
        assert_eq!(serde_json::to_value(f32_n_inf).unwrap(), json_n_inf);
        assert_eq!(serde_json::to_value(f32_nan).unwrap(), json_nan);
        assert_eq!(serde_json::to_value(f32_other_nan).unwrap(), json_other_nan);

        assert_eq!(
            serde_json::from_value::<F32WithNanInf>(json_num).unwrap(),
            f32_num
        );
        assert_eq!(
            serde_json::from_value::<F32WithNanInf>(json_inf).unwrap(),
            f32_inf
        );
        assert_eq!(
            serde_json::from_value::<F32WithNanInf>(json_n_inf).unwrap(),
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
