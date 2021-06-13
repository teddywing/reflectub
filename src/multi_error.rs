// Copyright (c) 2021  Teddy Wing
//
// This file is part of Reflectub.
//
// Reflectub is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Reflectub is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Reflectub. If not, see <https://www.gnu.org/licenses/>.


use std::fmt;


/// Wraps a list of errors.
#[derive(Debug, thiserror::Error)]
pub struct MultiError {
    errors: Vec<anyhow::Error>,
}

impl fmt::Display for MultiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            self.errors
                .iter()
                .map(|e| format!("{:#}", e))
                .collect::<Vec<_>>()
                .join("\n"),
        )
    }
}

impl From<anyhow::Error> for MultiError {
    fn from(error: anyhow::Error) -> Self {
        MultiError { errors: vec![error] }
    }
}

impl From<Vec<anyhow::Error>> for MultiError {
    fn from(errors: Vec<anyhow::Error>) -> Self {
        MultiError { errors: errors }
    }
}
