/// Function with 6 levels of nested if statements - triggers S134
pub fn process_nested(value: i32) -> i32 {
    let mut result = value;

    if result > 0 {
        if result > 10 {
            if result > 20 {
                if result > 30 {
                    if result > 40 {
                        if result > 50 {
                            result = result * 2;
                        } else {
                            result = result + 1;
                        }
                    } else {
                        result = result - 1;
                    }
                } else {
                    result = result * 3;
                }
            } else {
                result = result / 2;
            }
        } else {
            result = result + 5;
        }
    } else {
        result = 0;
    }

    result
}
