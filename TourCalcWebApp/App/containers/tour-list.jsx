import React from 'react';
import ReactDOM from 'react-dom';
import AppState from './appstate.jsx'
import Cookies from 'js-cookie';
import { BrowserRouter as Router, Route, Switch, Link, Redirect } from 'react-router-dom';
import TourAdd from './tour-add.jsx'
import TourNameEdit from './tour-rename.jsx'

import Table from '@material-ui/core/Table';
import TableBody from '@material-ui/core/TableBody';
import TableCell from '@material-ui/core/TableCell';
import TableHead from '@material-ui/core/TableHead';
import TableFooter from '@material-ui/core/TableFooter';
import TablePagination from '@material-ui/core/TablePagination';
import TableRow from '@material-ui/core/TableRow';
import Button from '@material-ui/core/Button';


export default class TourList extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            isToursLoaded: false,
            tours: null

        }
    }
    page = 0
    rowsPerPage = 5

    code = ""

    componentDidMount() {
        document.title = "Tourcalc: List of tours"
        this.loadTours()
    }
    loadTours() {
        return AppState.loadTours(this, this.page * this.rowsPerPage, this.rowsPerPage, this.code);
    }
    
    render() {
        if (!this.state.isToursLoaded) {
            return <div>Loading Tours ...</div>
        } else {
            return (
                <div>
                    <Table border={0} stickyHeader>
                        <TableHead>
                            <TableRow>
                                <TableCell>Tours
                                </TableCell>
                                <TableCell>Mode: {this.props.authData.type}
                                    {this.props.authData.type === 'Master'
                                        ? <div>
                                            <input type="text" defaultValue={this.code} onChange={(e) => { this.code = e.target.value }} />
                                            <Button variant="outlined" onClick={() => { this.loadTours() }}>Filter by code</Button>
                                        </div>
                                        : ''
                                     }
                                </TableCell>
                                <TableCell>{
                                    this.props.authData.type === 'Master'
                                        ? <TourAdd buttonText="Add" actionButtonText="Add Tour" app={this} open={false} chooseCode={true}>
                                            <Button color='primary' variant='outlined'>Add Tour</Button>
                                        </TourAdd>
                                        : (
                                            this.state.tours.tours.length > 0
                                                ? <TourAdd buttonText="Add" actionButtonText="Add Tour" app={this} open={false} chooseCode={false}>
                                                    <Button color='primary' variant='outlined'>Add Tour</Button>
                                                </TourAdd>
                                                : <span />
                                        )
                                }</TableCell>
                                <TableCell>#
                                </TableCell>
                            </TableRow>
                        </TableHead>
                        <TableBody>
                            {
                                this.state.tours.tours.map((t, idx) => {
                                    return (
                                        <TableRow key={'row' + idx} hover>
                                            <TableCell>
                                            {
                                                /*this.props.authData.type === 'Master'*/ true ? (
                                                    <span key={'s' + idx} style={{ cursor: "pointer", borderStyle: 'ridge', fontSize: "xx-small" }} onClick={() => {
                                                        if (window.confirm('Sure to delete tour ' + t.name + ' (id: ' + t.id + ')?')) {
                                                            AppState.deleteTour(this, t.id)
                                                                .then(() => { AppState.loadTours(this); })
                                                        }
                                                    }}>X</span>) : <span />
                                                }
                                                <u key={'u' + idx}>
                                                    <TourNameEdit key={'te' + idx} tourid={t.id}
                                                        name={t.name} app={this}
                                                        open={false}
                                                        authData={this.props.authData}
                                                        buttonText='Rename' actionButtonText="Change name" /></u>
                                            </TableCell>
                                            <TableCell>
                                                {(idx + 1) + (this.page * this.rowsPerPage)}.<Link key={'l' + idx} to={'/tour/' + t.id}>{t.name}
                                                </Link>&nbsp;
                                                [<b>{t.persons.length}</b>;<b>{t.spendings.length}</b>;<b>{t.persons.filter(p => (p.receivedInCents - p.spentInCents) >= 0).length > 0 ? ((1 -
                                                    t.persons.filter(p => (p.receivedInCents - p.spentInCents) > 0).length * 1.0 /
                                                    t.persons.filter(p => (p.receivedInCents - p.spentInCents) >= 0).length)
                                                    * 100).toFixed(0) : 0
                                                }%</b>]
                                            </TableCell>
                                            <TableCell>
                                                <Button variant='outlined' onClick={() => { document.getElementById('TourJsonTextArea').value = JSON.stringify(t, null, 2); }}>JSON</Button>
                                            </TableCell>
                                            <TableCell>
                                                <Button color='secondary' variant='outlined' onClick={
                                                    () => {
                                                        //alert('id: ' + t.id + 'tourAccessCode' + ' val: ' + document.getElementById(t.id + 'tourAccessCode').value);
                                                        if (document.getElementById(t.id + 'tourAccessCode').value != '') {
                                                            AppState.addTourJson(this, t
                                                                , document.getElementById(t.id + 'tourAccessCode').value
                                                                , document.getElementById(t.id + 'tourName').value
                                                            )
                                                                .then(() => {
                                                                    this.loadTours()
                                                                })
                                                        } else {
                                                            alert('Please enter access code')
                                                        }
                                                    }
                                                }>Clone</Button>

                                                    {this.props.authData.type === 'Master' ?
                                                    <span>Access code:<input type="text" size="4" id={t.id + "tourAccessCode"} /></span>
                                                    : <span><input type="hidden" id={t.id + "tourAccessCode"} value='none'/></span>
                                                }
                                                &nbsp;New tour name: <input type="text" size="16" id={t.id + "tourName"} defaultValue={'Clone ' + t.name} />
                                            </TableCell>
                                  </TableRow>
                                    )
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

                    <hr />
                    Tour JSON:
                    <textarea id="TourJsonTextArea" style={{ width: "100%" }} rows="7" defaultValue="Here will be tour JSON"/>
                </div>
                )
        }
    }

};