use axhal::paging::MappingFlags;
use axhal::trap::PageFaultFlags;
use axhal::uspace::{ReturnReason, UserContext};
use axmm::AddrSpace;
use axtask::{AxTaskRef, TaskInner};
use memory_addr::VirtAddr;

use crate::syscall;

/// Wrapper to ensure `UserContext` is 16-byte aligned on the stack.
///
/// On x86_64, the CPU aligns RSP to 16 bytes when delivering interrupts
/// from user mode (ring 3 → ring 0).  `TSS.RSP0` is set to
/// `&uctx + sizeof(TrapFrame)`.  If that address is not 16-byte aligned,
/// the CPU adjusts RSP, causing all pushed values to be offset by 8 bytes
/// and the kernel-RSP restore to read the wrong value (triple fault).
///
/// `sizeof(TrapFrame) == 176 == 11 × 16`, so TSS.RSP0 inherits the
/// alignment of `&uctx`.  Forcing 16-byte alignment here guarantees
/// TSS.RSP0 is also 16-byte aligned on every architecture.
#[repr(C, align(16))]
struct AlignedUserContext(UserContext);

/// Spawn a user task that enters user space and handles traps.
///
/// This task implements **lazy (demand) paging** for the user stack:
/// - The stack area is registered with map_alloc(populate=false)
/// - When the user first touches the stack, a page fault occurs.
/// - The handler uses AddrSpace::handle_page_fault to allocate
///   and map the page on demand, then resumes execution.
pub fn spawn_user_task(
    mut uspace: AddrSpace,
    ustack_top: VirtAddr,
    _ustack_vaddr: VirtAddr,
) -> AxTaskRef {
    let page_table_root = uspace.page_table_root();

    // Create the user context: entry point, stack top, arg0=0
    let entry = crate::APP_ENTRY;
    let sp = ustack_top;

    let mut task = TaskInner::new(
        move || {
            let mut aligned_uctx = AlignedUserContext(UserContext::new(entry, sp, 0));

            ax_println!(
                "Enter user space: entry={:#x}, ustack={:#x}, kstack={:#x}",
                entry,
                sp,
                axtask::current().kernel_stack_top().unwrap(),
            );

            loop {
                let reason = aligned_uctx.0.run();
                match reason {
                    ReturnReason::Syscall => {
                        if let Some(exit_code) = syscall::handle_syscall(&mut aligned_uctx.0) {
                            axtask::exit(exit_code as _);
                        }
                    }
                    ReturnReason::PageFault(vaddr, flags) => {
                        // Convert flags to PageFaultFlags for handle_page_fault
                        let access_flags = if flags.contains(MappingFlags::WRITE) {
                            PageFaultFlags::WRITE
                        } else if flags.contains(MappingFlags::EXECUTE) {
                            PageFaultFlags::EXECUTE
                        } else {
                            PageFaultFlags::READ
                        };

                        // Try to handle page fault using AddrSpace's handler
                        if uspace.handle_page_fault(vaddr, access_flags) {
                            ax_println!("handle page fault OK! addr={:#x}", vaddr);
                        } else {
                            ax_println!(
                                "{}: segmentation fault at {:#x}, exit!",
                                axtask::current().id_name(),
                                vaddr
                            );
                            axtask::exit(-1);
                        }
                    }
                    _ => {
                        ax_println!("Unexpected trap from user space: {:?}", reason);
                        axtask::exit(-1);
                    }
                }
            }
        },
        "userboot".into(),
        crate::KERNEL_STACK_SIZE,
    );

    // Set the page table root so the scheduler switches to user space
    // page table when this task is scheduled.
    task.ctx_mut().set_page_table_root(page_table_root);

    axtask::spawn_task(task)
}
