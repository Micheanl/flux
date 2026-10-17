package com.micheanl.flux

import com.mojang.logging.LogUtils
import net.neoforged.fml.common.Mod

@Mod(Flux.MOD_ID)
object Flux {
    const val MOD_ID = "flux"

    val LOGGER = LogUtils.getLogger()

    init {
        LOGGER.info("Loading {}", MOD_ID)
    }
}
