use log::info;
use std::io::{self, Read};

type Register = u32;
type Interval = u16;
type Index = usize;

const QE: [u16; 47] = [
    0x5601, 0x3401, 0x1801, 0x0ac1, 0x0521, 0x0221, 0x5601, 0x5401, 0x4801, 0x3801, 0x3001, 0x2401,
    0x1c01, 0x1601, 0x5601, 0x5401, 0x5101, 0x4801, 0x3801, 0x3401, 0x3001, 0x2801, 0x2401, 0x2201,
    0x1c01, 0x1801, 0x1601, 0x1401, 0x1201, 0x1101, 0x0ac1, 0x09c1, 0x08a1, 0x0521, 0x0441, 0x02a1,
    0x0221, 0x0141, 0x0111, 0x0085, 0x0049, 0x0025, 0x0015, 0x0009, 0x0005, 0x0001, 0x5601,
];
const NEXT_MPS: [Index; 47] = [
    1, 2, 3, 4, 5, 38, 7, 8, 9, 10, 11, 12, 13, 29, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26,
    27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 45, 46,
];
const NEXT_LPS: [Index; 47] = [
    1, 6, 9, 12, 29, 33, 6, 14, 14, 14, 17, 18, 20, 21, 14, 14, 15, 16, 17, 18, 19, 19, 20, 21, 22,
    23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 46,
];

const SWITCH_LM: [Index; 47] = [
    1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

// Table D-7 - Initial states for all contexts
const CONTEXT_UNIFORM: u8 = 46;
const CONTEXT_RUN_LENGTH: u8 = 3;
const CONTEXT_ALL_ZERO_NEIGHBORS: u8 = 4;
const CONTEXT_INITIAL: [u8; 19] = [
    CONTEXT_UNIFORM,
    CONTEXT_RUN_LENGTH,
    CONTEXT_ALL_ZERO_NEIGHBORS,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
];

fn print_c(n: u32) {
    println!("high {:016b}, low {:016b}", c_high(n), c_low(n));
}

fn c_high(n: u32) -> u16 {
    (n >> 16) as u16
}

fn c_low(n: u32) -> u16 {
    (n << 16 >> 16) as u16
}

// Inserting a new byte into the C register in the software-conventions decoder
// Figure J-3
fn bytein(
    bp: &mut dyn Iterator<Item = Result<u8, io::Error>>,
    c: &mut u32,
    ct: &mut u32,
) -> Result<u8, io::Error> {
    let mut b = bp.next().unwrap()?;

    // If B is a 0xFF byte, then B1 (the byte pointed to by BP+1) is tested
    if b == 0xFF {
        let b1 = bp.next().unwrap()?;

        // If B1 exceeds 0x8F, then B1 must be one of the marker codes.
        if b1 > 0x8F {
            // The marker code is interpreted as required, and the buffer
            // pointer remains pointed to the 0xFF prefix of the marker code
            // which terminates the arithmetically compressed image data.

            // 1-bits are then fed to the decoder until the decoding is
            // complete. This is shown by adding 0xFF00 to the C-register
            // and setting the bit counter CT to 8
            *ct = 8;

            todo!();
        }
        // If B1 is not a marker code, then BP is incremented to point
        // to the next byte which contains a stuffed bit.
        else {
            b = bp.next().unwrap()?;
            // The B is added to the C-register with an alignment such that
            // the stuff bit (which contains any carry) is added to the low
            // order bit of Chigh.
            // ORIGINAL: *c |= (b as u32) << 16;
            print_c(*c);
            *c = *c + 0xFF00 - ((b as u32) << 9);
            print_c(*c);
            *ct = 7;
        }
    }
    // If B is not a 0xFF byte, BP is incremented and the new value of B
    // is inserted into the << 7 high order 8 bits of Clow.
    else {
        //ORIGINAL: *c |= (b as u32) << 7;
        *c = *c + 0xFF00 - ((b as u32) << 8);
        *ct = 8;
    }

    Ok(b)
}

// Decoder LPS (Least Probable Symbol) path conditional exchange procedure
fn lps_exchange(a: &mut Interval, i: &mut Index, _d: &mut u8) -> Result<(), io::Error> {
    info!("LPS exchange");
    if *a < QE[*i] {
        *a = QE[*i];
        //*d = MPS(CX);
        *i = NEXT_MPS[*i];
    }
    todo!();
}

// Decoder MPS (Most Probable Symbol) path conditional exchange procedure
fn mps_exchange(_a: &mut Interval, _i: &mut Index, _d: &mut u8) -> Result<(), io::Error> {
    info!("MPS exchange");
    todo!();
}

fn renormd() -> Result<(), io::Error> {
    todo!();
}

pub fn decode<R: io::Read>(reader: &mut R) -> Result<(), io::Error> {
    // LPS - less probable symbol
    // MPS - more probable symbol

    // The coding operations are done using fixed precision integer arithmetic and using an integer representation of fractional values in which 0x8000 is equivalent to decimal 0,75.
    //

    // Carry-over into the external buffer is prevented by a bit stuffing procedure.

    //
    // Qe - current estimate of the LPS probability
    // CD - compressed image data
    // D - decision
    // CX - context
    //
    // sub-interval for the MPS = A - (Qe * A)
    // sub-interval for the LPS = Qe * A

    // TODO: Consider using struct for C-register

    // The code register is also doubled each time A is doubled.
    // Periodically – to keep C from overflowing – a byte of compressed
    // image data is removed from the high order bits of the C-register and
    // placed in an external compressed imagedata buffer.

    // decoding = CD + CX = D
    // encoding = CX + D = CD

    // I = index
    // The index to the current estimate is part of the information stored
    // for context CX. This index is used as the index to the table of
    // values in NMPS, which gives the next index for an MPS renormalizatio
    let mut i: Index = 0;

    // C-register - the concatenation of the Chigh and Clow registers
    //
    // Chigh and Clow can be thought of as one 32 bit C-register in that
    // renormalization of C shifts a bit of new data from the MSB of Clow
    // to the LSB of Chigh.
    //
    // Bits are packed into bytes from the MSB to the LSB.
    let mut c: Register = 0;
    let mut _c_high: [u8; 2] = [0; 2];
    let mut _c_low: [u8; 2] = [0; 2];

    // CT - bit counter
    let mut _ct: u32 = 0;

    // A - interval
    // The interval A is kept in the range 0,75 ≤ A < 1,5 by doubling it
    // whenever the integer value falls below 0x8000. 0x8000 is equivalent
    // to decimal 0,75
    let mut _a: Interval = 0;

    // INITDEC

    // BP is the buffer pointer
    // BPST is pointing to the first compressed byte
    let mut bp = reader.bytes();

    // The first byte of the compressed image data is shifted into the low
    // order byte of Chigh, and a new byte is then read in.
    let mut _b = bp.next().unwrap()?;
    c |= (_b as u32) << 16;

    // B is the byte pointed to by the compressed image data buffer pointer
    bytein(&mut bp, &mut c, &mut _ct)?;

    c <<= 7;
    _ct -= 7;
    _a = 0x8000;

    let mut d: u8 = 0;

    // Decode
    print_c(c);
    _a -= QE[i];
    if c_high(c) < _a {
        if _a & 0x8000 > 0 {
            mps_exchange(&mut _a, &mut i, &mut d)?;
            renormd()?;
        } else {
            //D = MPS(cx);
            todo!();
        }
    } else {
        lps_exchange(&mut _a, &mut i, &mut d)?;
        renormd()?;
    }

    Ok(())
}
