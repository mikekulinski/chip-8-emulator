use std::{fs};
use std::thread::sleep;
use std::time::{Duration, Instant};
use log::{debug, LevelFilter};
use minifb::{Key, Window, WindowOptions};

fn main() {
    simple_logger::SimpleLogger::new()
        .with_level(LevelFilter::Info)
        .init()
        .unwrap();

    debug!("Running CHIP-8 Emulator");
    let mut cpu = CPU::new();
    cpu.load_game("3-corax+.ch8");
    cpu.run_loop();
}

const WIDTH: usize = 64;
const HEIGHT: usize = 32;
const INSTRUCTIONS_PER_SECOND: usize = 700; // tune per-ROM if needed
const FRAMES_PER_SECOND: usize = 60;
const TIMER_HZ: usize = 60;

struct CPU {
    registers: Registers,
    memory: Memory,
    stack: Stack,
    display: Display
}

impl CPU {
    fn new() -> Self {
        Self{
            registers: Registers::new(),
            memory: Memory::new(),
            stack: Stack::new(),
            display: Display::new(),
        }
    }

    // Load game from disk into memory at location 0x200, since by convention the interpreter's
    // code would live from 0x000-0x1FF. Game files start after that.
    fn load_game(&mut self, file_name: &str) {
        let game_data = fs::read(format!("resources/{file_name}")).unwrap();
        self.memory.store_data(0x200, &game_data);
        self.registers.pc = 0x200;
        debug!("Loaded Game");
    }

    fn run_loop(&mut self) {
        // Outer loop runs at 60hz, as defined by
        while self.display.window.is_open() && !self.display.window.is_key_down(Key::Escape) {
            let frame_start = Instant::now();

            let instructions_this_frame = INSTRUCTIONS_PER_SECOND / FRAMES_PER_SECOND;
            for _ in 0..instructions_this_frame {
                let opcode = self.fetch();
                self.decode_and_execute(opcode);
            }

            // Redraw the display to the screen.
            self.display.draw();

            // Sleep until the next frame.
            let elapsed = (Instant::now() - frame_start).as_secs_f64();
            let duration_per_frame = 1.0 / FRAMES_PER_SECOND as f64;
            let sleep_duration = (duration_per_frame - elapsed).max(0.0);
            sleep(Duration::from_secs_f64(sleep_duration));
        }
    }

    fn fetch(&mut self) -> u16 {
        // Load the next instruction from the PC from memory.
        let instruction = u16::from_be_bytes(
            self.memory.load_data(self.registers.pc, 2).try_into().unwrap(),
        );
        self.registers.pc += 2;
        instruction
    }

    fn decode_and_execute(&mut self, opcode: Opcode) {
        debug!("Executing opcode: {:#04X}", opcode);

        // Extract out each nibble from the 2 byte opcode.
        let nibbles = (
            (opcode & 0xF000) >> 12,
            (opcode & 0x0F00) >> 8,
            (opcode & 0x00F0) >> 4,
            (opcode & 0x000F),
        );

        match nibbles {
            // (0x0, n1, n2, n3) => {}, // SYS addr. Ignored by modern interpreters.
            (0x0, 0x0, 0xE, 0x0) => self.clear_screen(), // CLS
            (0x0, 0x0, 0xE, 0xE) => self.ret(), // RET
            (0x1, n1, n2, n3) => self.jump(nibbles_to_addr(n1, n2, n3)), // JP addr
            (0x2, n1, n2, n3) => self.call(nibbles_to_addr(n1, n2, n3)), // CALL addr
            (0x3, x, k1, k2) => self.skip_if_eq(x as u8, nibbles_to_byte(k1, k2)), // SE Vx, byte
            (0x4, x, k1, k2) => self.skip_if_not_eq(x as u8, nibbles_to_byte(k1, k2)), // SNE Vx, byte
            (0x5, x, y, 0x0) => self.skip_if_reg_eq(x as u8, y as u8), // SE Vx, Vy
            (0x6, x, k1, k2) => self.load_vx_byte(x as u8, nibbles_to_byte(k1, k2)), // LD Vx, byte
            (0x7, x, k1, k2) => self.add_vx_byte(x as u8, nibbles_to_byte(k1, k2)), // ADD Vx, byte
            (0x8, x, y, 0x0) => self.load_vx_vy(x as u8, y as u8), // LD Vx, Vy
            (0x8, x, y, 0x1) => self.or_vx_vy(x as u8, y as u8), // OR Vx, Vy
            (0x8, x, y, 0x2) => self.and_vx_vy(x as u8, y as u8), // AND Vx, Vy
            (0x8, x, y, 0x3) => self.xor_vx_vy(x as u8, y as u8), // XOR Vx, Vy
            (0x8, x, y, 0x4) => self.add_vx_vy(x as u8, y as u8), // ADD Vx, Vy
            (0x8, x, y, 0x5) => self.sub_vx_vy(x as u8, y as u8), // SUB Vx, Vy
            (0x8, x, _y, 0x6) => self.shift_right(x as u8), // SHR Vx {, Vy}
            (0x8, x, y, 0x7) => self.sub_vy_vx(y as u8, x as u8), // SUBN Vx, Vy
            (0x8, x, _y, 0xE) => self.shift_left(x as u8), // SHL {, Vy}
            (0x9, x, y, 0x0) => self.skip_if_reg_not_eq(x as u8, y as u8), // SNE Vx, Vy
            (0xA, n1, n2, n3) => self.load_i(nibbles_to_addr(n1, n2, n3)), // LD I, addr
            (0xB, n1, n2, n3) => todo!(), // JP V0, addr
            (0xC, x, k1, k2) => todo!(), // RND Vx, byte
            (0xD, x, y, n) => self.draw(x as u8, y as u8, n), // DRW Vx, Vy, nibble
            (0xE, x, 0x9, 0xE) => todo!(), // SKP Vx
            (0xE, x, 0xA, 0x1) => todo!(), // SKNP Vx
            (0xF, x, 0x0, 0x7) => todo!(), // LD Vx, DT
            (0xF, x, 0x0, 0xA) => todo!(), // LD Vx, K
            (0xF, x, 0x1, 0x5) => todo!(), // LD DT, Vx
            (0xF, x, 0x1, 0x8) => todo!(), // LD ST, Vx
            (0xF, x, 0x1, 0xE) => self.add_i_vx(x as u8), // ADD I, Vx
            (0xF, x, 0x2, 0x9) => todo!(), // LD F, Vx
            (0xF, x, 0x3, 0x3) => self.store_bcd_into_memory(x as u8), // LD B, Vx
            (0xF, x, 0x5, 0x5) => self.store_regs_into_memory(x as u8), // LD [I], Vx
            (0xF, x, 0x6, 0x5) => self.load_from_memory_into_regs(x as u8), // LD Vx, [I]
            _ => panic!("Unknown instruction: {:X}{:X}{:X}{:X}", nibbles.0, nibbles.1, nibbles.2, nibbles.3),
        };
    }

    fn clear_screen(&mut self) {
        debug!("Cleared screen");
        self.display.clear()
    }

    fn ret(&mut self) {
        let addr = self.stack.pop();
        self.registers.pc = addr;
        self.registers.sp -= 1;
    }

    fn jump(&mut self, addr: u16) {
        debug!("Jumping to addr: {addr}");
        self.registers.pc = addr;
    }

    fn call(&mut self, addr: u16) {
        debug!("Calling addr: {addr}");
        self.registers.sp += 1;
        self.stack.push(self.registers.pc);
        self.registers.pc = addr;
    }

    fn skip_if_eq(&mut self, x: u8, byte: u8) {
        let val = self.registers.get_v_register(x);
        if val == byte {
            self.registers.pc += 2;
        }
    }

    fn skip_if_not_eq(&mut self, x: u8, byte: u8) {
        let val = self.registers.get_v_register(x);
        if val != byte {
            self.registers.pc += 2;
        }
    }

    fn skip_if_reg_eq(&mut self, x: u8, y: u8) {
        let val_x = self.registers.get_v_register(x);
        let val_y = self.registers.get_v_register(y);
        if val_x == val_y {
            self.registers.pc += 2;
        }
    }

    fn skip_if_reg_not_eq(&mut self, x: u8, y: u8) {
        let val_x = self.registers.get_v_register(x);
        let val_y = self.registers.get_v_register(y);
        if val_x != val_y {
            self.registers.pc += 2;
        }
    }

    fn load_i(&mut self, addr: u16) {
        debug!("Loading addr into i: {addr}");
        self.registers.i = addr;
    }

    fn load_vx_byte(&mut self, x: u8, byte: u8) {
        debug!("Loading byte into V{:01X}: {}", x, byte);
        self.registers.set_v_register(x, byte);
    }

    fn add_vx_byte(&mut self, x: u8, byte: u8) {
        debug!("Adding byte into V{:01X}: {}", x, byte);
        let val = self.registers.get_v_register(x);
        let new_val = val.wrapping_add(byte);
        self.registers.set_v_register(x, new_val);
    }

    fn load_vx_vy(&mut self, x: u8, y: u8) {
        let val_y = self.registers.get_v_register(y);
        self.registers.set_v_register(x, val_y);
    }

    fn or_vx_vy(&mut self, x: u8, y: u8) {
        let val_x = self.registers.get_v_register(x);
        let val_y = self.registers.get_v_register(y);
        self.registers.set_v_register(x, val_x | val_y);
    }

    fn and_vx_vy(&mut self, x: u8, y: u8) {
        let val_x = self.registers.get_v_register(x);
        let val_y = self.registers.get_v_register(y);
        self.registers.set_v_register(x, val_x & val_y);
    }

    fn xor_vx_vy(&mut self, x: u8, y: u8) {
        let val_x = self.registers.get_v_register(x);
        let val_y = self.registers.get_v_register(y);
        self.registers.set_v_register(x, val_x ^ val_y);
    }

    fn add_vx_vy(&mut self, x: u8, y: u8) {
        let val_x = self.registers.get_v_register(x);
        let val_y = self.registers.get_v_register(y);

        let (new_val, did_overflow) = val_x.overflowing_add(val_y);
        self.registers.set_v_register(x, new_val);
        self.registers.vf = if did_overflow {1} else {0};
    }

    fn sub_vx_vy(&mut self, x: u8, y: u8) {
        let val_x = self.registers.get_v_register(x);
        let val_y = self.registers.get_v_register(y);
        self.registers.set_v_register(x, val_x.wrapping_sub(val_y));
        self.registers.vf = if val_x > val_y {1} else {0};
    }

    fn sub_vy_vx(&mut self, y: u8, x: u8) {
        let val_x = self.registers.get_v_register(x);
        let val_y = self.registers.get_v_register(y);
        self.registers.set_v_register(x, val_y.wrapping_sub(val_x));
        self.registers.vf = if val_y > val_x {1} else {0};
    }

    fn shift_right(&mut self, x: u8) {
        let val_x = self.registers.get_v_register(x);
        self.registers.set_v_register(x, val_x >> 1);
        self.registers.vf = if val_x.trailing_ones() > 0 {1} else {0};
    }

    fn shift_left(&mut self, x: u8) {
        let val_x = self.registers.get_v_register(x);
        self.registers.set_v_register(x, val_x << 1);
        self.registers.vf = if val_x.leading_ones() > 0 {1} else {0};
    }

    fn add_i_vx(&mut self, x: u8) {
        let val_x = self.registers.get_v_register(x) as u16;
        let addr = self.registers.i;

        self.registers.i = val_x + addr;
    }

    fn store_bcd_into_memory(&mut self, x: u8) {
        // Store BCD representation of Vx in memory locations I, I+1, and I+2.
        // The interpreter takes the decimal value of Vx, and places the hundreds digit in memory
        // at location in I, the tens digit at location I+1, and the ones digit at location I+2.
        let val_x = self.registers.get_v_register(x);
        let addr = self.registers.i;
        let data = vec![
            (val_x / 100) % 10,  // hundreds digit
            (val_x / 10) % 10,   // tens digit
            val_x % 10,          // units digit
        ];
        self.memory.store_data(addr, &data);
    }

    fn store_regs_into_memory(&mut self, x: u8) {
        // Stores x registers worth of data into memory at the addr stored in I.
        let addr = self.registers.i;
        let mut data = Vec::with_capacity((x+1) as usize);
        for i in 0..=x {
            data.push(self.registers.get_v_register(i));
        }
        self.memory.store_data(addr, &data);
    }

    fn load_from_memory_into_regs(&mut self, x: u8) {
        // Loads x registers worth of data from memory, starting at I, into V(0-x).
        let addr = self.registers.i;
        let data = self.memory.load_data(addr, (x+1) as u16);
        for i in 0..=x {
            self.registers.set_v_register(i, data[i as usize]);
        }
    }

    fn draw(&mut self, x: u8, y: u8, n: u16) {
        let x = self.registers.get_v_register(x) % WIDTH as u8;
        let y = self.registers.get_v_register(y) % HEIGHT as u8;
        let sprite = self.memory.load_data(self.registers.i, n);

        debug!("Drawing sprite to the display: {:?}, X: {}, Y: {}", sprite, x, y);

        // Go through each byte in the sprite and apply it to the display.
        let mut any_turned_off = false;
        for j in 0..sprite.len() {
            let byte = sprite[j];
            let new_y = y + j as u8;
            if new_y >= HEIGHT as u8 {
                break
            }
            for i in 0..8 {
                let new_x = x + i;
                if new_x >= WIDTH as u8 {
                    break
                }

                let val = bit_at_index(byte, i);
                let turned_off = self.display.draw_pixel(new_x, new_y, val);
                if turned_off {
                    debug!("Something turned off at {}, {}", new_x, new_y);
                    any_turned_off = turned_off
                }
            }
        }

        // Set the flag register if we've turned any pixels off.
        self.registers.vf = if any_turned_off {1} else {0};
    }
}

fn nibbles_to_byte(k1: u16, k2: u16) -> u8 {
    ((k1 << 4) | k2) as u8
}

fn nibbles_to_addr(n1: u16, n2: u16, n3: u16) -> u16 {
    (n1 << 8) | (n2 << 4) | n3
}

fn bit_at_index(byte: u8, i: u8) -> bool {
    // The index provided is for the index from left-to-right, so we need to shift
    // 7-i digits so we properly handle each index correctly.
    ((byte >> (7-i)) & 1) != 0
}

type Opcode = u16;

struct Registers {
    // General Purpose registers
    v0: u8,
    v1: u8,
    v2: u8,
    v3: u8,
    v4: u8,
    v5: u8,
    v6: u8,
    v7: u8,
    v8: u8,
    v9: u8,
    va: u8,
    vb: u8,
    vc: u8,
    vd: u8,
    ve: u8,
    vf: u8, // Used as a flag register for some instructions.

    // Timer registers
    dt: u8, // Delay timer
    st: u8, // Sound timer

    // Special purpose registers.
    pc: u16, // Program counter
    sp: u8, // Stack pointer
    i: u16, // Index register (used for storing pointers to memory)
}

impl Registers {
    fn new() -> Self {
        Self {
            v0: 0,
            v1: 0,
            v2: 0,
            v3: 0,
            v4: 0,
            v5: 0,
            v6: 0,
            v7: 0,
            v8: 0,
            v9: 0,
            va: 0,
            vb: 0,
            vc: 0,
            vd: 0,
            ve: 0,
            vf: 0,
            dt: 0,
            st: 0,
            pc: 0,
            sp: 0,
            i: 0,
        }
    }

    fn set_v_register(&mut self, x: u8, value: u8) {
        match x {
            0x0 => self.v0 = value,
            0x1 => self.v1 = value,
            0x2 => self.v2 = value,
            0x3 => self.v3 = value,
            0x4 => self.v4 = value,
            0x5 => self.v5 = value,
            0x6 => self.v6 = value,
            0x7 => self.v7 = value,
            0x8 => self.v8 = value,
            0x9 => self.v9 = value,
            0xa => self.va = value,
            0xb => self.vb = value,
            0xc => self.vc = value,
            0xd => self.vd = value,
            0xe => self.ve = value,
            0xf => self.vf = value,
            _ => panic!("Unknown register: V{:X}", x),
        }
    }

    fn get_v_register(&self, x: u8) -> u8 {
        match x {
            0x0 => self.v0,
            0x1 => self.v1,
            0x2 => self.v2,
            0x3 => self.v3,
            0x4 => self.v4,
            0x5 => self.v5,
            0x6 => self.v6,
            0x7 => self.v7,
            0x8 => self.v8,
            0x9 => self.v9,
            0xa => self.va,
            0xb => self.vb,
            0xc => self.vc,
            0xd => self.vd,
            0xe => self.ve,
            0xf => self.vf,
            _ => panic!("Unknown register: V{:X}", x),
        }
    }
}

/*
The first 512 bytes, from 0x000 to 0x1FF, are where the original interpreter was located,
and should not be used by programs.
*/
struct Memory {
    data: [u8; 4096]
}

impl Memory {
    fn new() -> Self {
        let mut mem = Self{
            data: [0; 4096]
        };
        mem.initialize_font();
        mem
    }

    // We need to store the font somewhere in memory. It's been convention
    // to use this font, and to store it starting at 0x050.
    fn initialize_font(&mut self) {
        let font = vec![
            0xF0, 0x90, 0x90, 0x90, 0xF0, // 0
            0x20, 0x60, 0x20, 0x20, 0x70, // 1
            0xF0, 0x10, 0xF0, 0x80, 0xF0, // 2
            0xF0, 0x10, 0xF0, 0x10, 0xF0, // 3
            0x90, 0x90, 0xF0, 0x10, 0x10, // 4
            0xF0, 0x80, 0xF0, 0x10, 0xF0, // 5
            0xF0, 0x80, 0xF0, 0x90, 0xF0, // 6
            0xF0, 0x10, 0x20, 0x40, 0x40, // 7
            0xF0, 0x90, 0xF0, 0x90, 0xF0, // 8
            0xF0, 0x90, 0xF0, 0x10, 0xF0, // 9
            0xF0, 0x90, 0xF0, 0x90, 0x90, // A
            0xE0, 0x90, 0xE0, 0x90, 0xE0, // B
            0xF0, 0x80, 0x80, 0x80, 0xF0, // C
            0xE0, 0x90, 0x90, 0x90, 0xE0, // D
            0xF0, 0x80, 0xF0, 0x80, 0xF0, // E
            0xF0, 0x80, 0xF0, 0x80, 0x80  // F
        ];
        self.store_data(0x050, &font);
    }

    fn store_data(&mut self, addr: u16, data: &[u8]) {
        let addr = addr as usize;
        self.data[addr..addr + data.len()].copy_from_slice(data);
    }

    fn load_data(&self, addr: u16, n: u16) -> &[u8] {
        let addr = addr as usize;
        let n = n as usize;
        self.data[addr..addr+n].as_ref()
    }
}

struct Stack {
    // The stack is an array of 16 16-bit values, used to store the address that the interpreter
    // should return to when finished with a subroutine. Chip-8 allows for up to 16 levels of
    // nested subroutines.
    data: [u16; 16],
    // The index of where to put the next data.
    i: usize,
}

impl Stack {
    fn new() -> Self {
        Self{
            data: [0; 16],
            i: 0,
        }
    }

    fn push(&mut self, addr: u16) {
        if self.i >= self.data.len() {
            panic!("Stack overflow!");
        }
        self.data[self.i] = addr;
        self.i += 1;
    }

    fn pop(&mut self) -> u16 {
        if self.i == 0 {
            panic!("Stack underflow!");
        }
        // Update the index accordingly and return the value.
        self.i -= 1;
        self.data[self.i]
    }
}

struct Display {
    window: Window,
    // 64 x 32 pixels in the display of black/white.
    pixels: [bool; WIDTH * HEIGHT],
}

const BLACK: u32 = 0x000000;
const WHITE: u32 = 0xFFFFFF;

impl Display {
    fn new() -> Self {
        let mut window = Window::new(
            "Test - ESC to exit",
            WIDTH,
            HEIGHT,
            WindowOptions {
                resize: true,
                ..WindowOptions::default()
            }
        ).unwrap_or_else(|e| {
                panic!("{}", e);
            });
        window.set_target_fps(FRAMES_PER_SECOND);

        Self {
            window,
            pixels: [false; WIDTH * HEIGHT],
        }
    }

    // Turn all the pixels to false to reset the screen.
    fn clear(&mut self) {
        self.pixels = [false; WIDTH * HEIGHT]
    }

    // XOR's the value with the pixels' current value.
    // Returns true if a pixel was turned off.
    fn draw_pixel(&mut self, x: u8, y: u8, new: bool) -> bool {
        let x = x as usize;
        let y = y as usize;

        let prev = self.pixels[y * WIDTH + x];
        self.pixels[y * WIDTH + x] = prev ^ new;

        prev == true && new == true
    }

    fn draw(&mut self) {
        let buf = self.pixels
            .map(|p| if p {WHITE} else {BLACK});
        self.window
            .update_with_buffer(&buf, WIDTH, HEIGHT)
            .unwrap();
    }
}
