import React from 'react';
import ReactDOM from 'react-dom';
import AppState from './appstate.jsx'
import Button from '@material-ui/core/Button';

import Table from '@material-ui/core/Table';
import TableBody from '@material-ui/core/TableBody';
import TableCell from '@material-ui/core/TableCell';
import TableHead from '@material-ui/core/TableHead';
import TableFooter from '@material-ui/core/TableFooter';
import TablePagination from '@material-ui/core/TablePagination';
import TableRow from '@material-ui/core/TableRow';

import { BrowserRouter as Router, Route, Switch, Redirect, Link, withRouter } from 'react-router-dom';


export default class ChooseTourVersion extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            tours: null,
            isToursLoaded: false,
            tour: null,
            redirecting: false
        }
    }
    page = 0;
    rowsPerPage = 5
    componentWillReceiveProps(props) {
        //alert('rprops')
        this.setState({ tour: props.tour, redirecting: false });
        this.loadTours();
    }

    componentDidMount() {
        //alert('ddd')
        this.loadTours();
    }
    loadTours() {
        AppState.loadTourVersions(this, this.props.tour.id, this.page * this.rowsPerPage, this.rowsPerPage);
    }
    render() {
        if (!this.props.tour.isVersion) {
            if (!this.state.isToursLoaded) return <span>Loading versions ...</span>
            if (this.state.redirecting) return <span>Redirecting to version ...</span>
            return (
                <span>
                    History:
                    {this.state.tours.tours.length > 0 ?
                        <Table>
                            <TableHead>
                                <TableRow>
                                    <TableCell>
                                        Version
                                </TableCell>
                                    <TableCell>
                                        When
                                </TableCell>
                                    <TableCell>
                                        What
                                </TableCell>
                                </TableRow>
                            </TableHead>
                            <TableBody>
                                {this.state.tours.tours.map((t, idx) => {
                                    return (<TableRow key={t.id}>
                                        <TableCell>
                                            <b>{(this.state.tours.totalCount - ((idx) + (this.page * this.rowsPerPage)))}</b>
                                        </TableCell>
                                        <TableCell>
                                            {

                                                (new Date(t.dateVersioned).getFullYear() + "") + '-' +
                                                (new Date(t.dateVersioned).getMonth() + 1 + "").padStart(2, '0') + '-' +
                                                (new Date(t.dateVersioned).getDate() + "").padStart(2, '0') + ' ' +

                                                (new Date(t.dateVersioned).getHours() + "").padStart(2, '0') + ':' +
                                                (new Date(t.dateVersioned).getMinutes() + "").padStart(2, '0') + ':' +
                                                (new Date(t.dateVersioned).getSeconds() + "").padStart(2, '0')
                                            }
                                        </TableCell>
                                        <TableCell>
                                            <Link to={'/tour/' + t.id + '/persons'} onClick={() => { AppState.refreshMainApp() }}>
                                                {
                                                    'Before ' + t.versionComment
                                                }
                                            </Link>
                                        </TableCell>
                                    </TableRow>)
                                })
                                }
                            </TableBody>
                            <TableFooter>
                                <TableRow>
                                    <TablePagination count={this.state.tours.totalCount}
                                        onChangePage={
                                            (e, p) => {
                                                //alert('p:' + p);
                                                this.page = p;
                                                this.loadTours()
                                            }
                                        }
                                        onChangeRowsPerPage={(e) => {
                                            //alert('r:' + e.target.value)
                                            this.rowsPerPage = e.target.value;
                                            this.page = 0
                                            this.loadTours()
                                        }}
                                        page={this.page} rowsPerPage={this.rowsPerPage}
                                        rowsPerPageOptions={[5, 10, 25]}
                                    />
                                </TableRow>
                            </TableFooter>
                        </Table>
                        : <div style={{ textAlign: "center" }}><b>No history for the tour yet</b></div>
                        }
                </span>
            )
        } else {
            return (
                <span>
                    Version <b>{
                        
                        (new Date(this.props.tour.dateVersioned).getFullYear() + "") + '-' +
                        (new Date(this.props.tour.dateVersioned).getMonth() + 1 + "").padStart(2, '0') + '-' +
                        (new Date(this.props.tour.dateVersioned).getDate() + "").padStart(2, '0') + ' ' + 

                        (new Date(this.props.tour.dateVersioned).getHours() + "").padStart(2, '0') + ':' +
                        (new Date(this.props.tour.dateVersioned).getMinutes() + "").padStart(2, '0') + ':' +
                            (new Date(this.props.tour.dateVersioned).getSeconds() + "").padStart(2, '0')
                        + ': before ' + this.props.tour.versionComment
                    }
                    </b> <br />

                    <Button
                        color="primary" size='small' variant='outlined'
                        onClick={() => {
                        if (window.confirm('Are you sure to revert to this version?')) {
                            AppState.restoreTourVersion(this, this.props.tour.versionFor_Id, this.props.tour)
                            .then(() => { window.location = '/tour/' + this.props.tour.versionFor_Id + '/persons' })
                        }
                                }
                        }>Revert to this version</Button>
                </span>
            )
        }
    }
}
