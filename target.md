# Target Bugs

## [RUSTSEC-2018-0010](https://rustsec.org/advisories/RUSTSEC-2018-0010.html)

```rust
    pub fn sign<T: HasPrivate>(
        signcert: Option<&X509>,
        pkey: Option<&PKeyRef<T>>,
        certs: Option<&Stack<X509>>,
        data: Option<&[u8]>,
        flags: CMSOptions,
    ) -> Result<CmsContentInfo, ErrorStack> {
        unsafe {
            let signcert = match signcert {
                Some(cert) => cert.as_ptr(),
                None => ptr::null_mut(),
            };
            let pkey = match pkey {
                Some(pkey) => pkey.as_ptr(),
                None => ptr::null_mut(),
            };
            let data_bio_ptr = match data {
                // Temporary variable is immediately dropped
                Some(data) => MemBioSlice::new(data)?.as_ptr(),
                None => ptr::null_mut(),
            };
            let certs = match certs {
                Some(certs) => certs.as_ptr(),
                None => ptr::null_mut(),
            };

            let cms = cvt_p(ffi::CMS_sign(
                signcert,
                pkey,
                certs,
                data_bio_ptr,
                flags.bits(),
            ))?;

            Ok(CmsContentInfo::from_ptr(cms))
        }
    }
```

## [RUSTSEC-2019-0016](https://rustsec.org/advisories/RUSTSEC-2019-0016.html)

```rust
impl From<Buffer> for Vec<u8> {
    fn from(buffer: Buffer) -> Vec<u8> {
        let mut slice = Buffer::allocate(buffer.len);
        let len = buffer.copy_to(&mut slice);

        unsafe {
            // slice is dropped after ptr is extracted
            Vec::from_raw_parts(slice.as_mut_ptr(), len, slice.len())
        }
    }
}
```

## [RUSTSEC-2018-0003](https://rustsec.org/advisories/RUSTSEC-2018-0003.html)

```rust
    pub fn insert_many<I: IntoIterator<Item=A::Item>>(&mut self, index: usize, iterable: I) {
        let iter = iterable.into_iter();
        if index == self.len() {
            return self.extend(iter);
        }

        let (lower_size_bound, _) = iter.size_hint();
        assert!(lower_size_bound <= std::isize::MAX as usize);  // Ensure offset is indexable
        assert!(index + lower_size_bound >= index);  // Protect against overflow
        self.reserve(lower_size_bound);

        unsafe {
            let old_len = self.len();
            assert!(index <= old_len);
            let ptr = self.as_mut_ptr().offset(index as isize);
            // this code temporarily copies elements to make room for new elements,
            // and if an iterator panics in the following line, double-free can happen
            ptr::copy(ptr, ptr.offset(lower_size_bound as isize), old_len - index);
            for (off, element) in iter.enumerate() {
                if off < lower_size_bound {
                    ptr::write(ptr.offset(off as isize), element);
                    let len = self.len() + 1;
                    self.set_len(len);
                } else {
                    // Iterator provided more elements than the hint.
                    assert!(index + off >= index);  // Protect against overflow.
                    self.insert(index + off, element);
                }
            }
            let num_added = self.len() - old_len;
            if num_added < lower_size_bound {
                // Iterator provided fewer elements than the hint
                ptr::copy(ptr.offset(lower_size_bound as isize), ptr.offset(num_added as isize), old_len - index);
            }
        }
    }
```

### Similar bug in SmallVec, but doesn't have RUSTSEC entry number since the code was never released
```rust
    pub fn from_elem(elem: A::Item, n: usize) -> Self {
        if n > A::size() {
            vec![elem; n].into()
        } else {
            unsafe {
                let mut arr: A = ::std::mem::uninitialized();
                let ptr = arr.ptr_mut();

                for i in 0..n as isize {
                    // if clone panics, the code will drop uninitialized memory
                    ::std::ptr::write(ptr.offset(i), elem.clone());
                }

                SmallVec {
                    capacity: n,
                    data: SmallVecData::from_inline(arr),
                }
            }
        }
    }
```

## [RUSTSEC-2019-0021](https://rustsec.org/advisories/RUSTSEC-2019-0021.html)

```rust
impl<A, M: ArrayLength<A>, N: ArrayLength<GenericArray<A, M>>> Matrix<A, M, N> {
    #[inline] fn map_elements<B, F: FnMut(A) -> B>(self, mut f: F) -> Matrix<B, M, N>
      where M: ArrayLength<B>, N: ArrayLength<GenericArray<B, M>> {
        let Matrix(a) = self;
        let _wrapper = mem::ManuallyDrop::new(a);
        let mut c: GenericArray<GenericArray<B, M>, N> = unsafe { mem::uninitialized() };
        for i in 0..N::to_usize() { for j in 0..M::to_usize() { unsafe {
            // can double-free if f panics
            ptr::write(&mut c[i][j], f(ptr::read(&_wrapper[i][j])))
        }}}
        Matrix(c)
    }
}

#[inline]
fn zip_elements<A, B, C, M, N, F: FnMut(A, B) -> C>(Matrix(a): Matrix<A, M, N>,
                                                    Matrix(b): Matrix<B, M, N>,
                                                    mut f: F) -> Matrix<C, M, N>
  where M: ArrayLength<A> + ArrayLength<B> + ArrayLength<C>,
        N: ArrayLength<GenericArray<A, M>> + ArrayLength<GenericArray<B, M>> +
           ArrayLength<GenericArray<C, M>> {
    let mut c: GenericArray<GenericArray<C, M>, N> = unsafe { mem::uninitialized() };
    let mut wrapper = mem::ManuallyDrop::new(c);
    for i in 0..N::to_usize() { for j in 0..M::to_usize() { unsafe {
        // can double-free if f panics
        ptr::write(&mut wrapper[i][j], f(ptr::read(&a[i][j]), ptr::read(&b[i][j])))
    } } }
    mem::forget((a, b));
    Matrix(mem::ManuallyDrop::into_inner(wrapper))
}
```

## [RUSTSEC-2019-0012](https://rustsec.org/advisories/RUSTSEC-2019-0012.html)

```rust
    /// Re-allocate to set the capacity to `max(new_cap, inline_size())`.
    ///
    /// Panics if `new_cap` is less than the vector's length.
    pub fn grow(&mut self, new_cap: usize) {
        unsafe {
            let (ptr, &mut len, cap) = self.triple_mut();
            let unspilled = !self.spilled();
            assert!(new_cap >= len);
            if new_cap <= self.inline_size() {
                if unspilled {
                    return;
                }
                self.data = SmallVecData::from_inline(mem::uninitialized());
                ptr::copy_nonoverlapping(ptr, self.data.inline_mut().ptr_mut(), len);
                // semantic bug? capacity should be length for inline vector
                // self.capacity = len;
            } else if new_cap != cap {
                let mut vec = Vec::with_capacity(new_cap);
                let new_alloc = vec.as_mut_ptr();
                mem::forget(vec);
                ptr::copy_nonoverlapping(ptr, new_alloc, len);
                self.data = SmallVecData::from_heap(new_alloc, len);
                self.capacity = new_cap;
                if unspilled {
                    return;
                }
            }
            // shouldn't deallocate when new_cap == cap
            /*
            else {
                return;
            }
            */
            deallocate(ptr, cap);
        }
    }
```
