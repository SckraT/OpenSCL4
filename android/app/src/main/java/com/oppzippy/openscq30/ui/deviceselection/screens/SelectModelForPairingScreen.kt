package com.oppzippy.openscq30.ui.deviceselection.screens

import androidx.annotation.StringRes
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.oppzippy.openscq30.R
import com.oppzippy.openscq30.lib.bindings.deviceModels
import com.oppzippy.openscq30.lib.bindings.translateDeviceModel
import com.oppzippy.openscq30.ui.deviceselection.splitBrand
import com.oppzippy.openscq30.ui.sound.DeviceArtOrIcon
import com.oppzippy.openscq30.ui.sound.DeviceKind
import com.oppzippy.openscq30.ui.sound.deviceArt

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SelectModelForPairingScreen(onModelSelected: (String) -> Unit, onBackClick: () -> Unit) {
    var searchQuery by rememberSaveable { mutableStateOf("") }
    var selectedKind by rememberSaveable { mutableStateOf<DeviceKind?>(null) }
    val keyboard = LocalSoftwareKeyboardController.current
    val largeText = LocalDensity.current.fontScale > 1.4f
    val models = remember { deviceModels().map { it to translateDeviceModel(it) } }
    val kinds = remember(models) {
        DeviceKind.entries.filter { kind -> models.any { deviceArt(it.first)?.kind == kind } }
    }
    val filteredModels = models.filter { (model, name) ->
        (selectedKind == null || deviceArt(model)?.kind == selectedKind) &&
            (model.contains(searchQuery, true) || name.contains(searchQuery, true))
    }
    Scaffold(
        containerColor = MaterialTheme.colorScheme.background,
        topBar = {
            TopAppBar(
                title = {
                    Text(stringResource(R.string.sound_add_device), maxLines = 1, overflow = TextOverflow.Ellipsis)
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = MaterialTheme.colorScheme.background),
                navigationIcon = {
                    IconButton(onClick = onBackClick) {
                        Icon(
                            painterResource(R.drawable.arrow_back_24px),
                            contentDescription = stringResource(R.string.back),
                        )
                    }
                },
            )
        },
    ) { innerPadding ->
        LazyVerticalGrid(
            columns = if (largeText) GridCells.Fixed(1) else GridCells.Adaptive(minSize = 156.dp),
            modifier = Modifier.testTag("modelList").padding(innerPadding).fillMaxSize(),
            contentPadding = PaddingValues(start = 20.dp, end = 20.dp, top = 20.dp, bottom = 32.dp),
            horizontalArrangement = Arrangement.spacedBy(12.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            item(span = { GridItemSpan(maxLineSpan) }) {
                Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    Text(stringResource(R.string.sound_find_your_model), style = MaterialTheme.typography.headlineMedium)
                    Text(
                        stringResource(R.string.sound_find_your_model_description),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    OutlinedTextField(
                        modifier = Modifier.fillMaxWidth(),
                        value = searchQuery,
                        onValueChange = { searchQuery = it },
                        placeholder = { Text(stringResource(R.string.search)) },
                        leadingIcon = { Icon(painterResource(R.drawable.search_24px), contentDescription = null) },
                        shape = MaterialTheme.shapes.medium,
                        singleLine = true,
                        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                        keyboardActions = KeyboardActions(onSearch = { keyboard?.hide() }),
                    )
                    if (kinds.size > 1) {
                        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            KindChip(R.string.sound_kind_all, selectedKind == null) { selectedKind = null }
                            kinds.forEach { kind ->
                                KindChip(kind.label, selectedKind == kind) { selectedKind = kind }
                            }
                        }
                    }
                }
            }
            if (filteredModels.isEmpty()) {
                item(span = { GridItemSpan(maxLineSpan) }) {
                    Text(
                        stringResource(R.string.no_items_found),
                        modifier = Modifier.padding(20.dp),
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            } else {
                items(filteredModels, key = { it.first }) { (model, name) ->
                    ModelCard(model, name, onClick = { onModelSelected(model) })
                }
            }
        }
    }
}

@get:StringRes
private val DeviceKind.label: Int
    get() = when (this) {
        DeviceKind.Earbuds -> R.string.sound_kind_earbuds
        DeviceKind.OverEar -> R.string.sound_kind_over_ear
        DeviceKind.OpenEar -> R.string.sound_kind_open
        DeviceKind.Speaker -> R.string.sound_kind_speakers
    }

@Composable
private fun KindChip(@StringRes label: Int, selected: Boolean, onClick: () -> Unit) {
    FilterChip(selected = selected, onClick = onClick, label = { Text(stringResource(label)) })
}

@Composable
private fun ModelCard(model: String, name: String, onClick: () -> Unit) {
    Surface(
        onClick = onClick,
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(24.dp),
        color = MaterialTheme.colorScheme.surfaceContainerLowest,
    ) {
        Column(Modifier.padding(8.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Box(
                Modifier.fillMaxWidth().height(120.dp)
                    .background(MaterialTheme.colorScheme.secondaryContainer, RoundedCornerShape(18.dp)),
                contentAlignment = Alignment.Center,
            ) {
                DeviceArtOrIcon(model, Modifier.size(108.dp))
            }
            Column(Modifier.padding(start = 8.dp, end = 8.dp, bottom = 6.dp)) {
                Text(
                    splitBrand(name).second,
                    style = MaterialTheme.typography.titleSmall,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(
                    model.removePrefix("Soundcore"),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}
