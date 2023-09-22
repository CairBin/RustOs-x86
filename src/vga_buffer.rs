use core::fmt;
use lazy_static::lazy_static; //延迟初始化
use spin::Mutex; //自旋锁
use volatile::Volatile; //引入Volatile类型，该类型会告诉编译器优化写入Buffer会产生负效应

const BUFFER_HEIGHT: usize = 25;
const BUFFER_WIDTH: usize = 80;

/// ## 说明
/// VGA颜色枚举类型
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Color {
    Black = 0,
    Blue,
    Green,
    Cyan,
    Red,
    Magenta,
    Brown,
    LightGray,
    DarkGray,
    LightBlue,
    LightGreen,
    LightCyan,
    LightRed,
    Pink,
    Yellow,
    White,
}

/// ## 说明
/// ColorCode 颜色代码字节包装类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)] //确保类型和它的单个成员有相同的内存布局
struct ColorCode(u8);

impl ColorCode {
    /// ## 函数说明
    /// ColorCode构造函数
    /// ## 参数
    ///  * `foreground:Color` - 前景色
    ///  * `background:Color` - 背景色
    fn new(foreground: Color, background: Color) -> ColorCode {
        ColorCode((background as u8) << 4 | (foreground as u8))
    }
}

/// ## 说明
/// 屏幕字符
/// ## 成员
/// * `ascii_character:u8` - ascii字符
/// * `color_code:ColorCode` - 颜色
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)] //按照C语言约定的顺序布局
struct ScreenChar {
    ascii_character: u8,
    color_code: ColorCode,
}

/// ## 说明
/// VGA缓冲区
///
/// ## 成员
///  * `chars` - 缓冲区容器
#[repr(transparent)]
struct Buffer {
    chars: [[Volatile<ScreenChar>; BUFFER_WIDTH]; BUFFER_HEIGHT],
}

/// ## 说明
/// Writer类型写屏幕最后一行，并在一行写满或者接受换行符'\n'所有字符向上移动一行
///
/// ## 成员
/// * `column_position` - 跟踪最后一行位置
/// * `color_code` - 前景色和背景色
/// * `buffer` - VGA字符缓冲区的可变借用
pub struct Writer {
    column_position: usize,
    color_code: ColorCode,
    buffer: &'static mut Buffer,
}

impl Writer {
    /// ## 函数说明
    /// 打印字符，检测行是否已满，满则换行
    /// 如果是换行符，调用new_line方法换行
    /// 如果不是换行则打印字符
    ///
    /// ## 参数
    ///
    /// * `byte` - 被打印的字符
    ///
    /// ## 用法
    ///
    /// ```rust
    /// Writer.write_byte('x');
    /// ```

    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            byte => {
                //检查是否行已满，是则换行
                if self.column_position >= BUFFER_WIDTH {
                    self.new_line();
                }

                let row = BUFFER_HEIGHT - 1;
                let col = self.column_position;

                let color_code = self.color_code;

                self.buffer.chars[row][col].write(ScreenChar {
                    ascii_character: byte,
                    color_code,
                });

                self.column_position += 1;
            }
        }
    }

    /// ## 函数说明
    /// 换行方法，本质上向上移动一行
    ///
    /// ## 用法
    ///
    /// ```rust
    /// Writer.new_line();
    /// ```
    fn new_line(&mut self) {
        for row in 1..BUFFER_HEIGHT {
            for col in 0..BUFFER_WIDTH {
                let charc = self.buffer.chars[row][col].read();
                self.buffer.chars[row - 1][col].write(charc)
            }
        }

        self.clear_row(BUFFER_HEIGHT - 1);
        self.column_position = 0;
    }

    /// ## 函数说明
    /// 清除屏幕指定行，本质上用空白符覆盖
    /// ## 参数
    /// * `row` - 行号
    ///
    /// ## 用法
    /// ```rust
    /// Writer.clear_row(0);
    /// ```
    fn clear_row(&mut self, row: usize) {
        // 空白字符
        let blank = ScreenChar {
            ascii_character: b' ',
            color_code: self.color_code,
        };

        //覆盖整行
        for col in 0..BUFFER_WIDTH {
            self.buffer.chars[row][col].write(blank);
        }
    }

    /// ## 函数说明
    /// 通过调用循环调用write_byte方法打印字符串
    ///
    /// ## 参数
    /// * `str` - 被打印的字符串
    ///
    /// ## 用法
    /// ```rust
    /// Writer.write_string("Genshin,Starting");
    /// ```
    pub fn write_string(&mut self, str: &str) {
        for byte in str.bytes() {
            match byte {
                // 可以是能打印的 ASCII 码字节，也可以是换行符
                0x20..=0x7e | b'\n' => self.write_byte(byte),
                // 不包含在上述范围之内的字节
                _ => self.write_byte(0xfe),
            }
        }
    }
}

//支持Rust提供的格式化宏
impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

//全局静态接口
lazy_static! {
    pub static ref WRITER: Mutex<Writer> = Mutex::new(Writer {
        column_position: 0,
        color_code: ColorCode::new(Color::Yellow, Color::Black),
        buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
    });
}

/* -------------------print宏实现------------------ */

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::vga_buffer::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;

    //在Mutex被锁定时禁用中断，防止死锁
    use x86_64::instructions::interrupts;
    interrupts::without_interrupts(|| {
        WRITER.lock().write_fmt(args).unwrap();
    });
}

/* ---------------测试------------------ */

#[test_case]
fn test_println_simple() {
    println!("test_println_simple output");
}

#[test_case]
fn test_println_many() {
    for _ in 0..200 {
        println!("test_println_many output");
    }
}

#[test_case]
fn test_println_output() {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;

    let s = "Some test string that fits on a single line";
    //避免死锁，禁用中断
    interrupts::without_interrupts(|| {
        let mut writer = WRITER.lock(); //显示加锁
        writeln!(writer, "\n{}", s).expect("writeln failed"); //prinln!改为writer!绕开输出必须加锁的限制

        // use crate::serial_println;
        // for i in &writer.buffer.chars[BUFFER_HEIGHT - 2] {
        //     serial_println!(
        //         "{},{}",
        //         i.read().ascii_character,
        //         char::from(i.read().ascii_character)
        //     );
        // }

        for (i, c) in s.chars().enumerate() {
            let screen_char = writer.buffer.chars[BUFFER_HEIGHT - 2][i].read();
            //serial_println!("{},{}", char::from(screen_char.ascii_character), c);
            assert_eq!(char::from(screen_char.ascii_character), c);
        }
    });
}
