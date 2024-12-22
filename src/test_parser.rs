use std::fs::File;
use std::io::{self, BufRead, BufReader, Error, Write};
use std::path::Path;

use anyhow::{bail, Result};

// There are the following test directives according to the L3 writeup
// test return i program must execute correctly and return i
// test div-by-zero program must compile but raise SIGFPE
// test abort program must compile and run but raise SIGABRT
// test memerror program must compile and run and raise SIGUSR2
// test error program must fail to compile due to an L3 source error
// test typecheck program must typecheck correctly (see below)
// test compile
#[derive(Debug, PartialEq)]
pub enum TestResult {
    Ret(i32),
    DivByZero,
    Abort,
    MemError,
    SourceError, // TODO: I think this is just general parser error
    TypeCheck,
    Compile,
}

pub fn get_test_result<P>(p: P) -> Result<TestResult>
where
    P: AsRef<Path>,
{
    let file = File::open(p)?;
    let mut reader = BufReader::new(file);
    let first_line = get_line(&mut reader)?;

    parse_line(&first_line)
}

fn get_line<R>(mut handle: R) -> Result<String, io::Error>
where
    R: BufRead,
{
    let mut input = String::new();

    // TODO: handle empty files
    if 0 == handle.read_line(&mut input)? {
        std::process::exit(0);
    }

    Ok(input)
}

fn parse_line(first_line: &str) -> Result<TestResult> {
    let words: Vec<_> = first_line.split_whitespace().collect();

    if words.len() < 2 {
        bail!("Expected test directive instead got: {first_line}")
    }

    if words[0] != "//test" {
        bail!("Expected test directive to begin with //test instead got: {first_line}")
    }

    use TestResult::*;
    match words[1] {
        "return" => {
            if words.len() != 3 {
                bail!("Expected return test directive to have integer instead got: {first_line}")
            }

            let int_result: i32 = words[2].parse()?;

            Ok(Ret(int_result))
        }
        "div-by-zero" => Ok(DivByZero),
        "abort" => Ok(Abort),
        "memerror" => Ok(MemError),
        "error" => Ok(SourceError),
        "typecheck" => Ok(TypeCheck),
        "compile" => Ok(Compile),
        r => bail!("Expected a test directive return | div-by-zero | abort | memerror | error | typecheck | compile instead got: {r}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test1() {
        let first_line = "//test return 52";
        println!("RET {:?}", parse_line(&first_line));
        assert!(matches!(parse_line(&first_line), Ok(TestResult::Ret(52))));
    }
}
