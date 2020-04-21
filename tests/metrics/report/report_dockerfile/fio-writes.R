#!/usr/bin/env Rscript
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# Display details for 'fio' random writes storage IO tests.


library(ggplot2)	# ability to plot nicely
library(gridExtra)	# So we can plot multiple graphs together
suppressMessages(suppressWarnings(library(ggpubr)))	# for ggtexttable
suppressMessages(library(jsonlite))			# to load the data
suppressMessages(suppressWarnings(library(tidyr)))	# for gather
library(tibble)

testnames=c(
	"fio-randwrite-128",
	"fio-randwrite-256",
	"fio-randwrite-512",
	"fio-randwrite-1k",
	"fio-randwrite-2k",
	"fio-randwrite-4k",
	"fio-randwrite-8k",
	"fio-randwrite-16k",
	"fio-randwrite-32k",
	"fio-randwrite-64k"
	)

data2=c()
all_ldata=c()
all_ldata2=c()
stats=c()
rstats=c()
rstats_names=c()


# Where to store up the stats for the tables
write_bw_stats=c()
write_iops_stats=c()
write_lat95_stats=c()
write_lat99_stats=c()

# For each set of results
for (currentdir in resultdirs) {
	bw_dirstats=c()
	iops_dirstats=c()
	lat95_dirstats=c()
	lat99_dirstats=c()
	# Derive the name from the test result dirname
	datasetname=basename(currentdir)

	for (testname in testnames) {
		fname=paste(inputdir, currentdir, testname, '.json', sep="")
		if ( !file.exists(fname)) {
			#warning(paste("Skipping non-existent file: ", fname))
			next
		}

		# Import the data
		fdata=fromJSON(fname)
		# De-nest the test specific named data
		fdata=fdata[[testname]]

		blocksize=fdata$Raw$'global options'$bs

		# Extract the latency data - it comes as a table of percentiles, so
		# we have to do a little work...
		clat=data.frame(clat_ns=fdata$Raw$jobs[[1]]$write$clat_ns$percentile)

		# Generate a clat data set with 'clean' percentile numbers so
		# we can sensibly plot it later on.
		clat2=clat
		colnames(clat2)<-sub("clat_ns.", "", colnames(clat2))
		colnames(clat2)<-sub("0000", "", colnames(clat2))
		ldata2=gather(clat2)
		colnames(ldata2)[colnames(ldata2)=="key"] <- "percentile"
		colnames(ldata2)[colnames(ldata2)=="value"] <- "ms"
		ldata2$ms=ldata2$ms/1000000	#ns->ms
		ldata2=cbind(ldata2, runtime=rep(datasetname, length(ldata2$percentile)))
		ldata2=cbind(ldata2, blocksize=rep(blocksize, length(ldata2$percentile)))

		# Pull the 95 and 99 percentiles for the boxplot diagram.
		# Our values fall more in the range of ms...
		pc95data=tibble(percentile=clat$clat_ns.95.000000/1000000)
		pc95data=cbind(pc95data, runtime=rep(paste(datasetname, "95pc", sep="-"), length(pc95data$percentile)))
		pc99data=tibble(percentile=clat$clat_ns.99.000000/1000000)
		pc99data=cbind(pc99data, runtime=rep(paste(datasetname, "99pc", sep="-"), length(pc95data$percentile)))
		ldata=rbind(pc95data, pc99data)
		ldata=cbind(ldata, blocksize=rep(blocksize, length(ldata$percentile)))

		# We want total bandwidth, so that is the sum of the bandwidths
		# from all the write 'jobs'.
		mdata=data.frame(write_bw_mps=as.numeric(sum(fdata$Raw$jobs[[1]]$write$bw)/1024))
		mdata=cbind(mdata, iops_tot=as.numeric(sum(fdata$Raw$jobs[[1]]$write$iops)))
		mdata=cbind(mdata, runtime=rep(datasetname, length(mdata[, "write_bw_mps"]) ))
		mdata=cbind(mdata, blocksize=rep(blocksize, length(mdata[, "write_bw_mps"]) ))

		# Extract the stats tables
		bw_dirstats=rbind(bw_dirstats, round(mdata$write_bw_mps, digits=1))
		# Rowname hack to get the blocksize recorded
		rownames(bw_dirstats)[nrow(bw_dirstats)]=blocksize

		iops_dirstats=rbind(iops_dirstats, round(mdata$iops_tot, digits=1))
		rownames(iops_dirstats)[nrow(iops_dirstats)]=blocksize

		# And do the 95 and 99 percentiles as tables as well
		lat95_dirstats=rbind(lat95_dirstats, round(mean(clat$clat_ns.95.000000)/1000000, digits=1))
		rownames(lat95_dirstats)[nrow(lat95_dirstats)]=blocksize
		lat99_dirstats=rbind(lat99_dirstats, round(mean(clat$clat_ns.99.000000)/1000000, digits=1))
		rownames(lat99_dirstats)[nrow(lat99_dirstats)]=blocksize

		# Store away as single sets
		data2=rbind(data2, mdata)
		all_ldata=rbind(all_ldata, ldata)
		all_ldata2=rbind(all_ldata2, ldata2)
	}

	# Collect up for each dir we process into a column
	write_bw_stats=cbind(write_bw_stats, bw_dirstats)
	colnames(write_bw_stats)[ncol(write_bw_stats)]=datasetname

	write_iops_stats=cbind(write_iops_stats, iops_dirstats)
	colnames(write_iops_stats)[ncol(write_iops_stats)]=datasetname

	write_lat95_stats=cbind(write_lat95_stats, lat95_dirstats)
	colnames(write_lat95_stats)[ncol(write_lat95_stats)]=datasetname
	write_lat99_stats=cbind(write_lat99_stats, lat99_dirstats)
	colnames(write_lat99_stats)[ncol(write_lat99_stats)]=datasetname
}

# To get a nice looking table, we need to extract the rownames into their
# own column
write_bw_stats=cbind(Bandwidth=rownames(write_bw_stats), write_bw_stats)
write_bw_stats=cbind(write_bw_stats, Units=rep("MB/s", nrow(write_bw_stats)))

write_iops_stats=cbind(IOPS=rownames(write_iops_stats), write_iops_stats)
write_iops_stats=cbind(write_iops_stats, Units=rep("IOP/s", nrow(write_iops_stats)))

write_lat95_stats=cbind('lat 95pc'=rownames(write_lat95_stats), write_lat95_stats)
write_lat95_stats=cbind(write_lat95_stats, Units=rep("ms", nrow(write_lat95_stats)))
write_lat99_stats=cbind('lat 99pc'=rownames(write_lat99_stats), write_lat99_stats)
write_lat99_stats=cbind(write_lat99_stats, Units=rep("ms", nrow(write_lat99_stats)))

# lineplot of total bandwidth across blocksizes.
write_bw_line_plot <- ggplot() +
	geom_line( data=data2, aes(blocksize, write_bw_mps, group=runtime, color=runtime)) +
	ylim(0, NA) +
	ggtitle("Random Write total bandwidth") +
	xlab("Blocksize") +
	ylab("Bandwidth (MiB/s)") +
	theme(
		axis.text.x=element_text(angle=90),
		legend.position=c(0.35,0.8),
		legend.title=element_text(size=5),
		legend.text=element_text(size=5),
		legend.background = element_rect(fill=alpha('blue', 0.2))
	)

# lineplot of IOPS across blocksizes
write_iops_line_plot <- ggplot() +
	geom_line( data=data2, aes(blocksize, iops_tot, group=runtime, color=runtime)) +
	ylim(0, NA) +
	ggtitle("Random Write total IOPS") +
	xlab("Blocksize") +
	ylab("IOPS") +
	theme(
		axis.text.x=element_text(angle=90),
		legend.position=c(0.35,0.8),
		legend.title=element_text(size=5),
		legend.text=element_text(size=5),
		legend.background = element_rect(fill=alpha('blue', 0.2))
	)

# boxplot of 95 and 99 percentiles covering the parallel jobs, shown across
# the blocksizes.
write_clat_box_plot <- ggplot() +
	geom_boxplot( data=all_ldata, aes(blocksize, percentile, color=runtime)) +
	stat_summary( data=all_ldata, aes(blocksize, percentile, group=runtime, color=runtime), fun.y=mean, geom="line") +
	ylim(0, NA) +
	ggtitle("Random Write completion latency", subtitle="95&99 Percentiles, boxplot across jobs") +
	xlab("Blocksize") +
	ylab("Latency (ms)") +
	theme(axis.text.x=element_text(angle=90)) +
	# Use the 'paired' colour matrix as we are setting these up as pairs of
	# 95 and 99 percentiles, and it is much easier to visually group those to
	# each runtime if we use this colourmap.
	scale_colour_brewer(palette="Paired")


# completion latency line plot across the percentiles, for a specific blocksize only
# as otherwise the graph would be far too noisy.
which_blocksize='4k'
clat_line_subtitle=paste("For blocksize", which_blocksize, sep=" ")
single_blocksize=subset(all_ldata2, blocksize==which_blocksize)
clat_line=aggregate(
	single_blocksize$ms,
	by=list(
		percentile=single_blocksize$percentile,
		blocksize=single_blocksize$blocksize,
		runtime=single_blocksize$runtime
	),
	FUN=mean
)

clat_line$percentile=as.numeric(clat_line$percentile)

write_clat_line_plot <- ggplot() +
	geom_line( data=clat_line, aes(percentile, x, group=runtime, color=runtime)) +
	ylim(0, NA) +
	ggtitle("Random Write completion latency percentiles", subtitle=clat_line_subtitle) +
	xlab("Percentile") +
	ylab("Time (ms)") +
	theme(
		axis.text.x=element_text(angle=90),
		legend.position=c(0.35,0.8),
		legend.title=element_text(size=5),
		legend.text=element_text(size=5),
		legend.background = element_rect(fill=alpha('blue', 0.2))
	)

master_plot = grid.arrange(
	write_bw_line_plot,
	write_iops_line_plot,
	write_clat_box_plot,
	write_clat_line_plot,
	nrow=2,
	ncol=2 )

# A bit of an odd tweak to force a pagebreak between the pictures and
# the tables. This only works because we have a `results='asis'` in the Rmd
# R fragment.
cat("\n\n\\pagebreak\n")

write_bw_stats_plot = suppressWarnings(ggtexttable(write_bw_stats,
	theme=ttheme(base_size=10),
	rows=NULL
	))

write_iops_stats_plot = suppressWarnings(ggtexttable(write_iops_stats,
	theme=ttheme(base_size=10),
	rows=NULL
	))

write_lat95_stats_plot = suppressWarnings(ggtexttable(write_lat95_stats,
	theme=ttheme(base_size=10),
	rows=NULL
	))
write_lat99_stats_plot = suppressWarnings(ggtexttable(write_lat99_stats,
	theme=ttheme(base_size=10),
	rows=NULL
	))

# and then the statistics tables
stats_plot = grid.arrange(
	write_bw_stats_plot,
	write_iops_stats_plot,
	write_lat95_stats_plot,
	write_lat99_stats_plot,
	nrow=4,
	ncol=1 )
