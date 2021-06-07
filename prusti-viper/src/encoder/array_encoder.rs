// © 2021, ETH Zurich
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use rustc_middle::ty;
use std::{
    collections::HashMap,
    convert::TryInto,
};
use crate::encoder::{
    Encoder,
    errors::EncodingResult,
    builtin_encoder::BuiltinFunctionKind,
};
use prusti_common::{
    vir,
    vir_local,
};

/// The result of `ArrayEncoder::encode_array_types`. Contains types, type predicates and length of the given array type.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct EncodedArrayTypes<'tcx> {
    /// String to use as type predicate, e.g. Array$3$i32
    pub array_pred: String,
    /// Array type, e.g. TypedRef(Array$3$i32)
    pub array_ty: vir::Type,
    /// Element type, e.g. TypedRef(i32)
    pub elem_ty: vir::Type,
    /// Type of an element if stored as a (pure) snapshot value, e.g. Int
    pub elem_value_ty: vir::Type,
    /// The non-encoded element type as passed by rustc
    pub elem_ty_rs: ty::Ty<'tcx>,
    /// The length of the array, e.g. 3
    pub array_len: usize,
    /// The name of the lookup_pure function for this instance, e.g. "Array$3$i32$lookup_pure"
    pub lookup_pure_name: String,
}

impl<'tcx> EncodedArrayTypes<'tcx> {
    pub fn encode_lookup_pure_call(&self, array: vir::Expr, idx: vir::Expr) -> vir::Expr {
        vir::Expr::func_app(
            self.lookup_pure_name.clone(),
            vec![
                array,
                idx,
            ],
            vec![
                vir_local!{ self: {self.array_ty.clone()} },
                vir_local!{ idx: Int },
            ],
            self.elem_value_ty.clone(),
            vir::Position::default(),
        )
    }
}

/// The result of `ArrayEncoder::encode_slice_types`. Contains types and type predicates of the given array type.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct EncodedSliceTypes<'tcx> {
    /// String to use as type predicate, e.g. Slice$i32
    pub slice_pred: String,
    /// Slice type, e.g. TypedRef(Slice$i32)
    pub slice_ty: vir::Type,
    /// Element type, e.g. TypedRef(i32)
    pub elem_ty: vir::Type,
    /// Type of an element if stored as a (pure) snapshot value, e.g. Int
    pub elem_value_ty: vir::Type,
    /// The non-encoded element type as passed by rustc
    pub elem_ty_rs: ty::Ty<'tcx>,
    /// The name of the `lookup_pure` function for this instance, e.g. "Array$3$i32$lookup_pure"
    pub lookup_pure_name: String,
    /// The name of the slice length function for this instance, e.g. "Slice$i32$len"
    pub slice_len_name: String,
}

impl<'tcx> EncodedSliceTypes<'tcx> {
    pub fn encode_lookup_pure_call(&self, slice: vir::Expr, idx: vir::Expr) -> vir::Expr {
        vir::Expr::func_app(
            self.lookup_pure_name.clone(),
            vec![
                slice,
                idx,
            ],
            vec![
                vir_local!{ self: {self.slice_ty.clone()} },
                vir_local!{ idx: Int },
            ],
            self.elem_value_ty.clone(),
            vir::Position::default(),
        )
    }

    pub fn encode_slice_len_call(&self, slice: vir::Expr) -> vir::Expr {
        vir::Expr::func_app(
            self.slice_len_name.clone(),
            vec![
                slice,
            ],
            vec![
                vir_local!{ self: {self.slice_ty.clone()} },
            ],
            vir::Type::Int,
            vir::Position::default(),
        )
    }
}



pub struct ArrayTypesEncoder<'tcx> {
    array_types_cache: HashMap<ty::Ty<'tcx>, EncodedArrayTypes<'tcx>>,
    slice_types_cache: HashMap<ty::Ty<'tcx>, EncodedSliceTypes<'tcx>>,
}

impl<'p, 'v: 'p, 'tcx: 'v> ArrayTypesEncoder<'tcx> {
    pub fn new() -> Self {
        ArrayTypesEncoder {
            array_types_cache: HashMap::new(),
            slice_types_cache: HashMap::new(),
        }
    }

    /// Encode types, type predicates and builtin lookup functions required for handling slices of
    /// type `slice_ty`.
    ///
    /// Optionally pass a pre-encoded `vir::Type` for the element snapshot type. This is useful
    /// when calling `encode_slice_types` from e.g. inside the `SnapshotEncoder` where `Encoder`'s
    /// `snapshot_encoder` field is already borrowed, so `encoder.encode_snapshot_type` would
    /// result in a double borrow (i.e. panic).
    pub fn encode_slice_types(
        &mut self,
        encoder: &'p Encoder<'v, 'tcx>,
        slice_ty_rs: ty::Ty<'tcx>,
        pre_encoded_elem_snap_ty: Option<vir::Type>,
    ) -> EncodingResult<EncodedSliceTypes<'tcx>> {
        if let Some(cached) = self.slice_types_cache.get(&slice_ty_rs) {
            return Ok(cached.clone());
        }

        let slice_pred = encoder.encode_type_predicate_use(slice_ty_rs)?;
        let elem_ty_rs = if let ty::TyKind::Slice(elem_ty) = slice_ty_rs.kind() {
            elem_ty
        } else {
            unreachable!()
        };

        let elem_pred = encoder.encode_type_predicate_use(elem_ty_rs)?;

        let slice_ty = encoder.encode_type(slice_ty_rs)?;
        let elem_ty = encoder.encode_type(elem_ty_rs)?;

        let elem_value_ty = if let Some(pre_encoded) = pre_encoded_elem_snap_ty {
            pre_encoded
        } else {
            encoder.encode_snapshot_type(elem_ty_rs)?
        };

        let lookup_pure_name = encoder.encode_builtin_function_use(
            BuiltinFunctionKind::SliceLookupPure {
                slice_ty_pred: slice_pred.clone(),
                elem_ty_pred: elem_pred.clone(),
                return_ty: elem_value_ty.clone(),
            }
        );

        let slice_len_name = encoder.encode_builtin_function_use(
            BuiltinFunctionKind::SliceLen {
                slice_ty_pred: slice_pred.clone(),
                elem_ty_pred: elem_pred,
            }
        );

        let encoded = EncodedSliceTypes {
            slice_pred,
            slice_ty,
            elem_ty,
            elem_value_ty,
            elem_ty_rs,
            lookup_pure_name,
            slice_len_name,
        };
        self.slice_types_cache.insert(&slice_ty_rs, encoded.clone());

        Ok(encoded)
    }

    /// Encode types, type predicates and builtin lookup functions required for handling arrays of
    /// type `array_ty`.
    ///
    /// Optionally pass a pre-encoded `vir::Type` for the element snapshot type. This is useful
    /// when calling `encode_array_types` from e.g. inside the `SnapshotEncoder` where `Encoder`'s
    /// `snapshot_encoder` field is already borrowed, so `encoder.encode_snapshot_type` would
    /// result in a double borrow (i.e. panic).
    pub fn encode_array_types(
        &mut self,
        encoder: &'p Encoder<'v, 'tcx>,
        array_ty_rs: ty::Ty<'tcx>,
        pre_encoded_elem_snap_ty: Option<vir::Type>,
    ) -> EncodingResult<EncodedArrayTypes<'tcx>> {
        if let Some(cached) = self.array_types_cache.get(&array_ty_rs) {
            return Ok(cached.clone());
        }

        // type predicates
        let array_pred = encoder.encode_type_predicate_use(array_ty_rs)?;
        let (elem_ty_rs, len) = if let ty::TyKind::Array(elem_ty, len) = array_ty_rs.kind() {
            (elem_ty, len)
        } else {
            unreachable!()
        };
        let elem_pred = encoder.encode_type_predicate_use(elem_ty_rs)?;

        // types
        let array_ty = encoder.encode_type(array_ty_rs)?;
        let elem_ty = encoder.encode_type(elem_ty_rs)?;

        let elem_value_ty = if let Some(pre_encoded) = pre_encoded_elem_snap_ty {
            pre_encoded
        } else {
            encoder.encode_snapshot_type(elem_ty_rs)?
        };

        let array_len = encoder.const_eval_intlike(&len.val)?
            .to_u64().unwrap().try_into().unwrap();

        let lookup_pure_name = encoder.encode_builtin_function_use(
            BuiltinFunctionKind::ArrayLookupPure {
                array_ty_pred: array_pred.clone(),
                elem_ty_pred: elem_pred,
                array_len,
                return_ty: elem_value_ty.clone(),
            }
        );

        let encoded = EncodedArrayTypes {
            array_pred,
            array_ty,
            elem_ty,
            elem_value_ty,
            elem_ty_rs,
            array_len,
            lookup_pure_name,
        };
        self.array_types_cache.insert(&array_ty_rs, encoded.clone());

        Ok(encoded)
    }
}
