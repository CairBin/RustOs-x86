use super::align_up;
use super::Locked;
use alloc::alloc::{GlobalAlloc, Layout};
use core::mem;
use core::ptr;

struct ListNode {
    size: usize,
    next: Option<&'static mut ListNode>,
}

impl ListNode {
    /// ## 说明
    /// 创建一个链表结点
    /// ## 参数
    /// * `size` - 块的大小
    /// ## 用法
    /// ```rust
    /// ListNode::new(0);
    /// ```
    const fn new(size: usize) -> Self {
        ListNode { size, next: None }
    }

    /// ## 说明
    /// 获取起始地址
    ///
    /// ## 用法
    /// ```rust
    /// ListNode.start_addr();
    /// ```
    fn start_addr(&self) -> usize {
        self as *const Self as usize
    }

    /// ## 说明
    /// 获取终止地址
    /// ## 用法
    /// ```rust
    /// ListNode.end_addr();
    /// ```
    fn end_addr(&self) -> usize {
        self.start_addr() + self.size
    }
}

pub struct LinkedListAllocator {
    head: ListNode,
}

impl LinkedListAllocator {
    /// ## 说明
    /// 创建一个LinkedListAllocator并初始化头结点
    /// ## 用法
    /// ```rust
    /// LinkedListAllocator::new();
    /// ```
    pub const fn new() -> Self {
        Self {
            head: ListNode::new(0),
        }
    }

    /// ## 说明
    /// 使用给定的堆边界初始化分配器
    ///
    /// ## 参数
    /// * `heap_start` - 起始边界
    /// * `heap_size` - 堆大小
    ///
    /// ## 用法
    /// ```rust
    /// LinkedListAllocator.init(0,100);
    /// ```
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.add_free_region(heap_start, heap_size);
    }

    /// ## 说明
    /// 内存区域前插至链表
    ///
    /// ## 参数
    /// * `heap_start` - 起始边界
    /// * `heap_size` - 堆大小
    ///
    /// ## 用法
    /// ```rust
    /// LinkedListAllocator.add_free_region(0,100);
    /// ```
    unsafe fn add_free_region(&mut self, addr: usize, size: usize) {
        //确保释放的区域能容纳ListNode
        assert_eq!(align_up(addr, mem::align_of::<ListNode>()), addr);
        assert!(size >= mem::size_of::<ListNode>());

        //创建一个新的结点并尾插到链表
        let mut node = ListNode::new(size);
        node.next = self.head.next.take();
        let node_ptr = addr as *mut ListNode;
        node_ptr.write(node);
        self.head.next = Some(&mut *node_ptr)
    }

    /// ## 说明
    /// 查找并删除具有给定大小和对齐方式的空闲区域
    /// 返回列表节点的元组和分配的起始地址
    ///
    /// ## 参数
    /// * `size` - 空闲区域的指定大小
    /// * `align` - 对齐方式
    fn find_region(&mut self, size: usize, align: usize) -> Option<(&'static mut ListNode, usize)> {
        let mut current = &mut self.head;
        while let Some(ref mut region) = current.next {
            if let Ok(alloc_start) = Self::alloc_from_region(&region, size, align) {
                let next = region.next.take();
                let ret = Some((current.next.take().unwrap(), alloc_start));
                current.next = next;
                return ret;
            } else {
                current = current.next.as_mut().unwrap();
            }
        }

        None
    }

    /// ## 说明
    /// 分配指定大小和对齐方式的区域
    ///
    /// ## 参数
    /// * `size` - 大小
    /// * `align` - 对齐方式
    ///
    /// ## 用法
    /// ```rust
    /// LinkedListAllocator.alloc_from_region(100,10);
    /// ```
    fn alloc_from_region(region: &ListNode, size: usize, align: usize) -> Result<usize, ()> {
        let alloc_start = align_up(region.start_addr(), align);
        let alloc_end = alloc_start.checked_add(size).ok_or(())?;

        if alloc_end > region.end_addr() {
            return Err(()); //区域过小
        }

        let excess_size = region.end_addr() - alloc_end;
        if excess_size > 0 && excess_size < mem::size_of::<ListNode>() {
            return Err(());
        }

        Ok(alloc_start)
    }

    fn size_align(layout: Layout) -> (usize, usize) {
        let layout = layout
            .align_to(mem::align_of::<ListNode>())
            .expect("adjusting alignment failed")
            .pad_to_align();
        let size = layout.size().max(mem::size_of::<ListNode>());
        (size, layout.align())
    }
}

unsafe impl GlobalAlloc for Locked<LinkedListAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // 执行布局调整
        let (size, align) = LinkedListAllocator::size_align(layout);
        let mut allocator = self.lock();

        if let Some((region, alloc_start)) = allocator.find_region(size, align) {
            let alloc_end = alloc_start.checked_add(size).expect("overflow");
            let excess_size = region.end_addr() - alloc_end;
            if excess_size > 0 {
                allocator.add_free_region(alloc_end, excess_size);
            }

            alloc_start as *mut u8
        } else {
            ptr::null_mut()
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let (size, _) = LinkedListAllocator::size_align(layout);
        self.lock().add_free_region(ptr as usize, size)
    }
}
