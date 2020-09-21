// Can't be implemented as ext because of the `fail()` part
macro_rules! convert {
    ($err:expr) => {
        $err.map_err(|e| -> Box<dyn AnalysisError + '_> { Box::new(e) })
    };
}

/// Unwraps the value, add the value to `vec` and return immediately if error.
macro_rules! unwrap_or_return {
    ($vec:expr, $val:expr) => {
        match $val {
            Ok(val) => val,
            Err(e) => {
                $vec.push(Err(e));
                return;
            }
        }
    };
    ($vec:expr, $val:expr, return $return_val:expr) => {
        match $val {
            Ok(val) => val,
            Err(e) => {
                $vec.push(Err(e));
                return $return_val;
            }
        }
    };
}
