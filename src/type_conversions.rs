use crate::thread_affinity::CallbackLifetime;
use pyo3::prelude::*;
use pyo3::IntoPyObjectExt;
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::types::{PyAny, PyBool, PyByteArray, PyBytes, PyDict, PyFloat, PyIterator, PyList, PyInt, PyString, PyTuple};
use serde_json::Value;
use yrs::types::{Attrs, Change, EntryChange, Delta, Events, Path, PathSegment};
use yrs::{Any, Number, Out, TransactionMut, XmlOut};
use std::collections::{VecDeque, HashMap};
use std::sync::Arc;
use crate::text::{Text, TextEvent};
use crate::array::{Array, ArrayEvent};
use crate::map::{Map, MapEvent};
use crate::doc::Doc;
use crate::xml::{XmlElement, XmlEvent, XmlFragment, XmlText};

pub trait ToPython {
    fn into_py<'py>(self, py: Python<'py>) -> Bound<'py, PyAny>;
}

impl<T: ToPython> ToPython for Vec<T> {
    fn into_py<'py>(self, py: Python<'py>) -> Bound<'py, PyAny> {
        let elements = self.into_iter().map(|v| v.into_py(py));
        PyList::new(py, elements).unwrap().into_bound_py_any(py).unwrap()
    }
}

impl<T: ToPython> ToPython for VecDeque<T> {
    fn into_py<'py>(self, py: Python<'py>) -> Bound<'py, PyAny> {
        let elements = self.into_iter().map(|v| v.into_py(py));
        PyList::new(py, elements).unwrap().into_bound_py_any(py).unwrap()
    }
}

impl ToPython for Path {
    fn into_py<'py>(self, py: Python<'py>) -> Bound<'py, PyAny> {
        let result = PyList::empty(py);
        for segment in self {
            match segment {
                PathSegment::Key(key) => {
                    result.append(key.as_ref()).unwrap();
                }
                PathSegment::Index(idx) => {
                    result.append(idx).unwrap();
                }
            }
        }
        result.into_bound_py_any(py).unwrap()
    }
}

impl ToPython for Delta {
    fn into_py<'py>(self, py: Python<'py>) -> Bound<'py, PyAny> {
        let result = PyDict::new(py);
        match self {
            Delta::Inserted(value, attrs) => {
                let value = value.clone().into_py(py);
                result.set_item("insert", value).unwrap();

                if let Some(attrs) = attrs {
                    let attrs = (&*attrs).into_py(py);
                    result.set_item("attributes", attrs).unwrap();
                }
            }
            Delta::Retain(len, attrs) => {
                result.set_item("retain", len).unwrap();

                if let Some(attrs) = attrs {
                    let attrs = (&*attrs).into_py(py);
                    result.set_item("attributes", attrs).unwrap();
                }
            }
            Delta::Deleted(len) => {
                result.set_item("delete", len).unwrap();
            }
        }
        result.into_bound_py_any(py).unwrap()
    }
}

impl ToPython for Out {
    fn into_py<'py>(self, py: Python<'py>) -> Bound<'py, PyAny> {
        match self {
            Out::Any(v) => v.into_py(py),
            Out::YXmlText(v) => Py::new(py, XmlText::from(v)).unwrap().into_any().into_bound(py),
            Out::YText(v) => Py::new(py, Text::from(v)).unwrap().into_any().into_bound(py),
            Out::YArray(v) => Py::new(py, Array::from(v)).unwrap().into_any().into_bound(py),
            Out::YMap(v) => Py::new(py, Map::from(v)).unwrap().into_any().into_bound(py),
            Out::YDoc(v) => Py::new(py, Doc::from(v)).unwrap().into_any().into_bound(py),
            Out::YXmlElement(v) => Py::new(py, XmlElement::from(v)).unwrap().into_any().into_bound(py),
            Out::YXmlFragment(v) => Py::new(py, XmlFragment::from(v)).unwrap().into_any().into_bound(py),
            Out::UndefinedRef(_) => py.None().into_bound(py),
        }
    }
}

impl ToPython for XmlOut {
    fn into_py<'py>(self, py: Python<'py>) -> Bound<'py, PyAny> {
        match self {
            XmlOut::Element(xml_element_ref) => Py::new(py, XmlElement::from(xml_element_ref))
                .unwrap()
                .into_any()
                .into_bound(py),
            XmlOut::Fragment(xml_fragment_ref) => Py::new(py, XmlFragment::from(xml_fragment_ref))
                .unwrap()
                .into_any()
                .into_bound(py),
            XmlOut::Text(xml_text_ref) => {
                Py::new(py, XmlText::from(xml_text_ref)).unwrap().into_any().into_bound(py)
            }
        }
    }
}

impl<T> ToPython for Option<T>
where
    T: ToPython,
{
    fn into_py<'py>(self, py: Python<'py>) -> Bound<'py, PyAny> {
        self.map(|v| v.into_py(py)).unwrap_or_else(|| py.None().into_bound(py))
    }
}

impl ToPython for &'_ Attrs {
    fn into_py<'py>(self, py: Python<'py>) -> Bound<'py, PyAny> {
        let result = PyDict::new(py);
        for (key, value) in self.iter() {
            let key = key.as_ref();
            let value = Out::Any(value.clone()).into_py(py);
            result.set_item(key, value).unwrap();
        }
        result.into_bound_py_any(py).unwrap()
    }
}

impl ToPython for &Change {
    fn into_py<'py>(self, py: Python<'py>) -> Bound<'py, PyAny> {
        let result = PyDict::new(py);
        match self {
            Change::Added(values) => {
                let values: Vec<Bound<'py, PyAny>> =
                    values.into_iter().map(|v| v.clone().into_py(py)).collect();
                result.set_item("insert", values).unwrap();
            }
            Change::Removed(len) => {
                result.set_item("delete", len).unwrap();
            }
            Change::Retain(len) => {
                result.set_item("retain", len).unwrap();
            }
        }
        result.into_bound_py_any(py).unwrap()
    }
}

#[repr(transparent)]
pub struct EntryChangeWrapper<'a>(pub &'a EntryChange);

impl<'py, 'a> IntoPyObject<'py> for EntryChangeWrapper<'a> {
    type Target = PyDict;
    type Output = Bound<'py, Self::Target>;
    type Error = std::convert::Infallible;

    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        let result = PyDict::new(py);
        let action = "action";
        match self.0 {
            EntryChange::Inserted(new) => {
                let new_value = new.clone().into_py(py);
                result.set_item(action, "add").unwrap();
                result.set_item("newValue", new_value).unwrap();
            }
            EntryChange::Updated(old, new) => {
                let old_value = old.clone().into_py(py);
                let new_value = new.clone().into_py(py);
                result.set_item(action, "update").unwrap();
                result.set_item("oldValue", old_value).unwrap();
                result.set_item("newValue", new_value).unwrap();
            }
            EntryChange::Removed(old) => {
                let old_value = old.clone().into_py(py);
                result.set_item(action, "delete").unwrap();
                result.set_item("oldValue", old_value).unwrap();
            }
        }
        Ok(result)
    }
}

/// A list or a dict an [Any] array or map is being read into, with the values it has still to read
/// and, for a dict, the key of the value read last.
enum Reading<'a, 'py> {
    List(std::slice::Iter<'a, Any>, Vec<Bound<'py, PyAny>>),
    Dict(std::collections::hash_map::Iter<'a, String, Any>, Bound<'py, PyDict>, Option<&'a String>),
}

impl<'a, 'py> Reading<'a, 'py> {
    /// Holds `read`, the value read last if it is one of this container's, and hands back the value
    /// to read next, if this container holds another.
    fn next_after(&mut self, read: Option<Bound<'py, PyAny>>) -> Option<&'a Any> {
        match self {
            Reading::List(items, list) => {
                list.extend(read);
                items.next()
            }
            Reading::Dict(entries, dict, key) => {
                if let Some(read) = read {
                    dict.set_item(key.take().expect("a key for every value read"), read).unwrap();
                }
                let (next_key, value) = entries.next()?;
                *key = Some(next_key);
                Some(value)
            }
        }
    }

    fn into_object(self, py: Python<'py>) -> Bound<'py, PyAny> {
        match self {
            Reading::List(_, list) => PyList::new(py, list).unwrap().into_any(),
            Reading::Dict(_, dict, _) => dict.into_any(),
        }
    }
}

impl ToPython for Any {
    fn into_py<'py>(self, py: Python<'py>) -> Bound<'py, PyAny> {
        // Arrays and maps nest as deep as the bytes they were decoded from ran, so the lists and
        // dicts still being filled are kept here rather than on the native stack.
        let mut open: Vec<Reading<'_, 'py>> = Vec::new();
        let mut next = Some(&self);
        let mut read = None;
        loop {
            match next {
                Some(Any::Array(items)) => open.push(Reading::List(items.iter(), Vec::with_capacity(items.len()))),
                Some(Any::Map(entries)) => open.push(Reading::Dict(entries.iter(), PyDict::new(py), None)),
                Some(leaf) => read = Some(leaf_into_py(leaf, py)),
                None => {}
            }
            let Some(container) = open.last_mut() else {
                return read.expect("a value read before none is left open");
            };
            next = container.next_after(read.take());
            if next.is_none() {
                read = open.pop().map(|container| container.into_object(py));
            }
        }
    }
}

fn leaf_into_py<'py>(leaf: &Any, py: Python<'py>) -> Bound<'py, PyAny> {
    match leaf {
        Any::Bool(v) => PyBool::new(py, *v).into_bound_py_any(py).unwrap(),
        Any::Number(Number::Float(v)) => PyFloat::new(py, *v).into_bound_py_any(py).unwrap(),
        Any::Number(Number::Int(v)) => {
            if (Number::I64_MIN_SAFE_INTEGER..=Number::I64_MAX_SAFE_INTEGER).contains(v) {
                PyFloat::new(py, *v as f64).into_bound_py_any(py).unwrap()
            } else {
                v.into_pyobject(py).unwrap().into_bound_py_any(py).unwrap()
            }
        },
        Any::String(v) => v.as_ref().into_pyobject(py).unwrap().into_bound_py_any(py).unwrap(),
        Any::Buffer(v) => PyByteArray::new(py, v.as_ref()).into_bound_py_any(py).unwrap(),
        Any::Null | Any::Undefined => py.None().into_bound(py),
        Any::Array(_) | Any::Map(_) => unreachable!("an array or a map is read into a list or a dict"),
    }
}

pub fn py_to_any<'py>(value: &Bound<'py, PyAny>) -> Any {
    if value.is_none() {
        Any::Null
    } else if value.is_instance_of::<PyBytes>() {
        let v: &[u8] = value.extract().unwrap();
        Any::Buffer(v.into())
    } else if value.is_instance_of::<PyString>() {
        let v: &str = value.extract().unwrap();
        Any::String(v.into())
    } else if value.is_instance_of::<PyBool>() {
        let v: bool = value.extract().unwrap();
        Any::Bool(v)
    } else if value.is_instance_of::<PyInt>() {
        let v: i64 = value.extract().unwrap();
        if !(Number::I64_MIN_SAFE_INTEGER..=Number::I64_MAX_SAFE_INTEGER).contains(&v) {
            Any::Number(Number::Int(v))
        } else {
            Any::Number(Number::Float(v as f64))
        }
    } else if value.is_instance_of::<PyFloat>() {
        let v: f64 = value.extract().unwrap();
        Any::Number(Number::Float(v))
    } else if let Ok(v) = value.cast::<PyList>() {
        let mut items = Vec::new();
        for i in v.iter() {
            let a = py_to_any(&i);
            items.push(a);
        }
        Any::Array(items.into())
    } else if let Ok(v) = value.cast::<PyTuple>() {
        let mut items = Vec::new();
        for i in v.iter() {
            let a = py_to_any(&i);
            items.push(a);
        }
        Any::Array(items.into())
    } else if let Ok(val) = value.cast::<PyDict>() {
        let mut items: HashMap<String, Any> = HashMap::new();
        for (k, v) in val.iter() {
            let k = k.cast::<PyString>().unwrap().to_str().unwrap().to_string();
            let v = py_to_any(&v);
            items.insert(k, v);
        }
        Any::Map(items.into())
    } else {
        Any::Undefined
    }
}

/// Convert a Python object into a JSON-compatible [`Any`], used for `IdMap` attribute values
/// (see [`crate::id_map`]).
///
/// This reuses [`py_to_any`] for the Python->Any traversal, so attribute values follow the same
/// JS-number semantics as every other pycrdt value (small ints become `Any::Number`), and then
/// rejects anything with no JSON representation: `bytes` (-> `Any::Buffer`), non-finite floats
/// (`NaN`/`inf`), and unsupported types (-> `Any::Undefined`).
///
/// Note: a `dict` with non-string keys panics inside `py_to_any` before this validation runs (the
/// same as for any other pycrdt value), surfacing as a `PanicException` rather than a `TypeError`.
pub(crate) fn py_to_json_any(value: &Bound<'_, PyAny>) -> PyResult<Any> {
    let any = py_to_any(value);
    ensure_json_compatible(&any)?;
    Ok(any)
}

/// Reject `Any` values that have no JSON representation.
fn ensure_json_compatible(any: &Any) -> PyResult<()> {
    match any {
        Any::Null | Any::Bool(_) | Any::Number(Number::Int(_)) | Any::String(_) => Ok(()),
        Any::Number(Number::Float(n)) if n.is_finite() => Ok(()),
        Any::Number(_) => Err(PyValueError::new_err(
            "NaN and infinity cannot be used as attribute values",
        )),
        Any::Buffer(_) => Err(PyTypeError::new_err(
            "bytes cannot be used as a JSON attribute value",
        )),
        Any::Undefined => Err(PyTypeError::new_err(
            "attribute value must be JSON-serializable (None, bool, int, float, str, list, tuple, or dict)",
        )),
        Any::Array(items) => {
            for item in items.iter() {
                ensure_json_compatible(item)?;
            }
            Ok(())
        }
        Any::Map(entries) => {
            for (_key, value) in entries.iter() {
                ensure_json_compatible(value)?;
            }
            Ok(())
        }
    }
}

/// Render a JSON-compatible [`Any`] (see [`py_to_json_any`]) as a [`serde_json::Value`].
///
/// `serde_json::Map` is `BTreeMap`-backed (no `preserve_order` feature), so map keys are always
/// emitted in sorted order. This makes the output canonical and suitable for equality and hashing.
pub(crate) fn any_to_value(any: &Any) -> Value {
    match any {
        Any::Null | Any::Undefined | Any::Buffer(_) => Value::Null,
        Any::Bool(b) => Value::Bool(*b),
        Any::Number(Number::Int(i))
            if !(Number::I64_MIN_SAFE_INTEGER..=Number::I64_MAX_SAFE_INTEGER).contains(i) =>
        {
            Value::Number((*i).into())
        },
        // Yrs may encode exactly representable large integers as floats. Keep their
        // canonical JSON identical after decoding, including in nested attributes.
        Any::Number(Number::Float(f))
            if f.fract() == 0.0
                && (*f < Number::F64_MIN_SAFE_INTEGER || *f > Number::F64_MAX_SAFE_INTEGER)
                && *f >= i64::MIN as f64
                && *f < -(i64::MIN as f64) =>
        {
            Value::Number((*f as i64).into())
        },
        Any::Number(n) => serde_json::Number::from_f64(match n {
            Number::Int(i) => *i as f64,
            Number::Float(f) => *f,
        })
        .map(Value::Number)
        .unwrap_or(Value::Null),
        Any::String(s) => Value::String(s.to_string()),
        Any::Array(items) => Value::Array(items.iter().map(any_to_value).collect()),
        Any::Map(map) => Value::Object(map.iter().map(|(k, v)| (k.clone(), any_to_value(v))).collect()),
    }
}

/// Parse a [`serde_json::Value`] into an [`Any`]. Python number normalization is
/// applied by [`ToPython`] when the value is read.
pub(crate) fn value_to_any(value: &Value) -> Any {
    Any::from_json(&value.to_string()).expect("serde_json::Value renders to valid JSON for Any")
}

pub(crate) fn events_into_py<'py>(
    py: Python<'py>,
    txn: &TransactionMut,
    events: &Events,
    lifetime: CallbackLifetime,
) -> Bound<'py, PyList> {
    let py_events = events.iter().map(|event| match event {
        yrs::types::Event::Text(e_txt) => Py::new(py, TextEvent::new(e_txt, txn, lifetime.clone()))
            .unwrap()
            .into_bound_py_any(py)
            .unwrap(),
        yrs::types::Event::Array(e_arr) => {
            Py::new(py, ArrayEvent::new(e_arr, txn, lifetime.clone()))
                .unwrap()
                .into_bound_py_any(py)
                .unwrap()
        }
        yrs::types::Event::Map(e_map) => Py::new(py, MapEvent::new(e_map, txn, lifetime.clone()))
            .unwrap()
            .into_bound_py_any(py)
            .unwrap(),
        yrs::types::Event::XmlFragment(e_xml) => unsafe {
            Py::new(
                py,
                XmlEvent::from_xml_event(e_xml, txn, py, lifetime.clone()),
            )
            .unwrap()
            .into_bound_py_any(py)
            .unwrap()
        },
        yrs::types::Event::XmlText(e_xml) => unsafe {
            Py::new(
                py,
                XmlEvent::from_xml_text_event(e_xml, txn, py, lifetime.clone()),
            )
            .unwrap()
            .into_bound_py_any(py)
            .unwrap()
        },
    });
    PyList::new(py, py_events).unwrap()
}

/// Converts an iterator of k,v tuples to an [`Attrs`] map
pub(crate) fn py_to_attrs<'py>(
    pyobj: Bound<'py, PyIterator>,
) -> PyResult<Attrs> {
    pyobj.map(|res| res.and_then(|item| {
        let key = item.get_item(0)?.extract::<Bound<PyString>>()?;
        let value = item.get_item(1).map(|v| py_to_any(&v))?;
        Ok((Arc::from(key.to_str()?), value))
    })).collect::<PyResult<Attrs>>()
}
