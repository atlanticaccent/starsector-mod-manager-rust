#pragma once

#include <memory>

#include "bin/bit7z/include/bitextractor.hpp"

std::unique_ptr<bit7z::BitExtractor> new_extractor();

void extract(std::unique_ptr<bit7z::BitExtractor>, std::string, std::string);
