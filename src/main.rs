#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]

#[cfg(feature = "axstd")]
extern crate axstd as std;

#[cfg(feature = "axstd")]
extern crate alloc;

#[cfg(feature = "axstd")]
#[macro_use]
extern crate axlog;

#[cfg(feature = "axstd")]
extern crate axfs;
#[cfg(feature = "axstd")]
extern crate axio;

#[cfg(feature = "axstd")]
mod loader;
#[cfg(feature = "axstd")]
mod syscall;
#[cfg(feature = "axstd")]
mod task;

#[cfg(feature = "axstd")]
const USER_STACK_SIZE: usize = 0x10000;
#[cfg(feature = "axstd")]
const KERNEL_STACK_SIZE: usize = 0x40000; // 256 KiB
#[cfg(feature = "axstd")]
const APP_ENTRY: usize = 0x1000;

#[cfg_attr(feature = "axstd", unsafe(no_mangle))]
fn main() {
    #[cfg(feature = "axstd")]
    {
        use axhal::paging::MappingFlags;
        use memory_addr::va;

        // A new address space for user app using axmm::new_user_aspace().
        // User space: [0x0, 0x40_0000_0000) — 256GB, below kernel space.
        let mut uspace = axmm::new_user_aspace(va!(0x0), 0x40_0000_0000).unwrap();

        // Load user app binary file into address space.
        if let Err(e) = loader::load_user_app("/sbin/origin", &mut uspace) {
            panic!("Cannot load app! {:?}", e);
        }

        // Init user stack with LAZY mapping:
        // Use map_alloc with populate=false to enable demand paging.
        // When user touches the stack, a page fault occurs, and the handler
        // allocates and maps the page on demand.
        let ustack_top = uspace.end();
        let ustack_vaddr = ustack_top - USER_STACK_SIZE;
        ax_println!(
            "Mapping user stack: {:#x?} -> {:#x?}",
            ustack_vaddr,
            ustack_top
        );

        uspace
            .map_alloc(
                ustack_vaddr,
                USER_STACK_SIZE,
                MappingFlags::READ | MappingFlags::WRITE | MappingFlags::USER,
                false, // populate=false: lazy/demand paging
            )
            .unwrap();

        ax_println!("New user address space: {:#x?}", uspace);

        // Let's kick off the user process.
        let user_task = task::spawn_user_task(uspace, ustack_top, ustack_vaddr);

        // Wait for user process to exit ...
        let exit_code = user_task.join();
        ax_println!("monolithic kernel exit [{:?}] normally!", exit_code);
    }
    #[cfg(not(feature = "axstd"))]
    {
        println!("This application requires the 'axstd' feature for lazy mapping execution.");
        println!("Run with: cargo xtask run [--arch <ARCH>]");
    }
}
